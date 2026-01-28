use crate::{
    constants::{ANCHOR_DISCRIMINATOR_SIZE}, 
    error::OrderBookError,
    state::{
        ForeignOrder, GLOBAL_SEED, NativeOrder, ORDER_SEED_PREFIX, Order, 
        OrderBookGlobal, OrderData, OrderStatus, OrderType, compute_order_id
    }, utils::{transfer_tokens_from_program}
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::portal::{
    constants::{AUTHORITY_SEED as PORTAL_AUTHORITY_SEED, GLOBAL_SEED as PORTAL_GLOBAL_SEED},
    cpi::{accounts::SendCancelReport, send_cancel_report},
    program::Portal,
    ID as PORTAL_ID
};

// Instructions related to cancelling orders
// Orders must be cancelled on their destination chain.
// From the perspective of the chain that this program is deployed on,
// there are two main flows:
// 1. cancelling a samechain order (i.e. current chain ID == origin chain ID == destination chain ID)
//   a. for orders that both originate on the current chain
//      and have the current chain as the destination
//      the order sender or recipient (before or on the deadline) or anyone (after the deadline)
//      initiates a cancel by executing `CancelNativeOrder`
// 2. cancelling a cross-chain order
//   a. for orders that have the current chain as the destination, (i.e. current chain ID == destination chain ID != origin chain ID)
//      the order recipient (before or on the deadline) or anyone (after the deadline)
//      initiates a cancel by executing `CancelForeignOrder`
//      this sends a cancel report back to the origin chain via a CPI to the Portal program
//   b. for orders that originate on the current chain, (i.e. current chain ID == origin chain ID != destination chain ID)
//      the relayer reports the cancel back to the origin chain by executing `ReportOrderCancel`
//      via the Portal program

// Handler Inputs
#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct CancelReport {
    pub order_id: [u8; 32],
    pub order_sender: [u8; 32],
    pub token_in: [u8; 32],
    pub amount_in_to_refund: u128
}

// Events
#[event]
pub struct RefundClaimed {
    pub order_id: [u8; 32],
    pub sender: Pubkey,
    pub amount: u64,
}

#[event]
pub struct OrderCancelled {
    pub order_id: [u8; 32],
}

#[event]
pub struct CancelReported {
    pub order_id: [u8; 32],
}

// Instruction Contexts and Handlers
#[event_cpi]
#[derive(Accounts)]
#[instruction(order_id: [u8; 32])]
pub struct CancelNativeOrder<'info> {
    pub signer: Signer<'info>,

    /// CHECK: The sender of the order, we don't read any data from here
    /// This does not have to be a signer, anyone can claim refunds on behalf of the sender
    #[account(address = order.data.sender @ OrderBookError::InvalidSender)]
    pub sender: UncheckedAccount<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_account.bump,
        constraint = !global_account.paused @ OrderBookError::ProgramPaused,
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(
        mut,
        seeds = [ORDER_SEED_PREFIX, order_id.as_ref()],
        bump = order.bump,
        constraint = order.data.dest_chain_id == global_account.chain_id @ OrderBookError::InvalidDestChainId,
        constraint = order.order_type == OrderType::Native @ OrderBookError::InvalidOrderType
    )]
    pub order: Account<'info, Order::<NativeOrder>>,

    #[account(
        address = order.data.token_in @ OrderBookError::InvalidTokenMint,
        mint::token_program = token_in_program
    )]
    pub token_in_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = token_in_mint,
        associated_token::authority = sender,
        associated_token::token_program = token_in_program,
    )]
    pub sender_token_in_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = token_in_mint,
        associated_token::authority = order,
        associated_token::token_program = token_in_program,
    )]
    pub order_token_in_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_in_program: Interface<'info, TokenInterface>,
}

impl CancelNativeOrder<'_> {
    fn validate(&self) -> Result<()> {
        let order = &self.order.data;

        // Validate the order has a valid status for cancellation
        if order.status != OrderStatus::Created {
            return err!(OrderBookError::InvalidOrderStatus);
        }

        let current_timestamp = Clock::get()?.unix_timestamp as u64;

        // Validate the order created_at time is not in the future
        require!(
            current_timestamp >= order.created_at,
            OrderBookError::InvalidCreatedAtTimestamp
        );

        // Validate the signer is either sender or recipient
        // if the fill deadline has not yet passed
        require!(
            current_timestamp > order.fill_deadline ||
            self.signer.key() == order.sender || // can use sender here because it's a native order
            self.signer.key() == Pubkey::new_from_array(order.recipient),
            OrderBookError::NotAuthorized
        );

        Ok(())
    }   

    #[access_control(ctx.accounts.validate())]
    pub fn handler(ctx: Context<Self>, order_id: [u8; 32]) -> Result<()> {
        let order = &mut ctx.accounts.order.data;

        // Don't have to subtract amount_in_refunded here because it is zero until cancelled
        let amount_in_remaining: u64 = order.amount_in
            .checked_sub(order.amount_in_released)
            .ok_or(OrderBookError::MathUnderflow)?
            .try_into()
            .map_err(|_| OrderBookError::MathOverflow)?;

        // Set the order status to Cancelled
        order.status = OrderStatus::Cancelled;
        order.amount_in_refunded += amount_in_remaining as u128;

        // Transfer the remaining tokens to the recipient
        // We do not check amount received here to avoid DoSing refunds
        if amount_in_remaining > 0 {
            transfer_tokens_from_program(
                &ctx.accounts.order_token_in_ata,
                &ctx.accounts.sender_token_in_ata,
                amount_in_remaining.try_into().map_err(|_| OrderBookError::MathOverflow)?,
                &ctx.accounts.token_in_mint,
                &ctx.accounts.order.to_account_info(),
                &[&[
                    ORDER_SEED_PREFIX,
                    order_id.as_ref(),
                    &[ctx.accounts.order.bump],
                ]],
                &ctx.accounts.token_in_program,
            )?;
        } else {
            return err!(OrderBookError::OrderFilled);
        }

        emit_cpi!(RefundClaimed {
            order_id,
            sender: ctx.accounts.sender.key(),
            amount: amount_in_remaining,
        });

        emit_cpi!(OrderCancelled {
            order_id,
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(order_id: [u8; 32], order_data: OrderData)]
pub struct CancelForeignOrder {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = global_account.bump,
        constraint = !global_account.paused @ OrderBookError::ProgramPaused,
        constraint = order_data.dest_chain_id == global_account.chain_id @ OrderBookError::InvalidDestChainId,
        constraint = order_data.origin_chain_id != global_account.chain_id @ OrderBookError::InvalidOriginChainId,
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(
        init_if_needed,
        payer = signer,
        space = ANCHOR_DISCRIMINATOR_SIZE + Order::<ForeignOrder>::INIT_SPACE,
        seeds = [ORDER_SEED_PREFIX, order_id.as_ref()],
        bump,
    )]
    pub order: Account<'info, Order::<ForeignOrder>>,

    pub portal_program: Program<'info, Portal>,

    /// CHECK: We validate the account seeds here
    /// The data is not used in this instruction
    /// We pass it into the CPI to the portal program
    #[account(
        mut,
        seeds = [PORTAL_GLOBAL_SEED],
        seeds::program = PORTAL_ID,
        bump,
    )]
    pub portal_global: UncheckedAccount<'info>,

    /// CHECK: We validate the seeds here
    /// The account holds no data and is used as a signer
    /// in the CPI to the portal program
    #[account(
        seeds = [PORTAL_AUTHORITY_SEED],
        seeds::program = PORTAL_ID,
        bump
    )]
    pub portal_authority: UncheckedAccount<'info>,

    /// CHECK: Bridge adapter program
    /// This is validated in the portal CPI
    pub bridge_adapter: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> CancelForeignOrder<'info> {
    fn validate(&self, order_id: [u8; 32], order_data: &OrderData) -> Result<()> {
        let order = &self.order.data;

        // Validate the order ID matches the order data
        let expected_order_id = compute_order_id(order_data);
    
        require!(
            order_id == expected_order_id,
            OrderBookError::InvalidOrderId
        );

        // Validate the order has a valid status for cancellation
        require!(
            order.status == OrderStatus::Created || order.status == OrderStatus::DoesNotExist, 
            OrderBookError::InvalidOrderStatus
        );

        let current_timestamp = Clock::get()?.unix_timestamp as u64;
        // Validate the order created_at time is not in the future
        require!(
            current_timestamp >= order_data.created_at,
            OrderBookError::InvalidCreatedAtTimestamp
        );

        // Validate the signer is recipient if the fill deadline has not yet passed
        require!(
            current_timestamp > order_data.fill_deadline ||
            self.signer.key() == Pubkey::new_from_array(order_data.recipient),
            OrderBookError::NotAuthorized
        );

        Ok(())
    }   

    #[access_control(ctx.accounts.validate(order_id, &order_data))]
    pub fn handler(ctx: Context<'_, '_, 'info, 'info, Self>, order_id: [u8; 32], order_data: OrderData) -> Result<()> {
        // If this is a new order, initialize it
        if ctx.accounts.order.data.status == OrderStatus::DoesNotExist {
            ctx.accounts.order.set_inner(Order::<ForeignOrder> {
                order_type: OrderType::Foreign,
                bump: ctx.bumps.order,
                data: ForeignOrder {
                    status: OrderStatus::Created,
                    amount_in_released: 0,
                    amount_out_filled: 0,
                    amount_in_refunded: 0
                }
            });
        } else {
            // Otherwise, validate the type of the order
            require!(
                ctx.accounts.order.order_type == OrderType::Foreign,
                OrderBookError::InvalidOrderType
            );
        }

        let order = &mut ctx.accounts.order.data;

        let amount_in_remaining = order_data.amount_in
            .checked_sub(order.amount_in_released)
            .ok_or(OrderBookError::MathUnderflow)?;

        // Set the order status to Cancelled and increment the refunded amount
        order.status = OrderStatus::Cancelled;
        order.amount_in_refunded += amount_in_remaining;

        // Send a cancel report message to the origin chain via the portal program
        send_cancel_report(
            CpiContext::new_with_signer(
                ctx.accounts.portal_program.to_account_info(),
                SendCancelReport {
                    sender: ctx.accounts.signer.to_account_info(),
                    order_book_global: ctx.accounts.global_account.to_account_info(),
                    portal_global: ctx.accounts.portal_global.to_account_info(),
                    portal_authority: ctx.accounts.portal_authority.to_account_info(),
                    bridge_adapter: ctx.accounts.bridge_adapter.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                },
                &[&[GLOBAL_SEED, &[ctx.accounts.global_account.bump]]],
            )
            .with_remaining_accounts(ctx.remaining_accounts.to_vec()),
            order_id, // order_id: [u8; 32],
            order_data.sender, // order_sender: [u8; 32],
            order_data.token_in, // token_in: [u8; 32],
            amount_in_remaining, // amount_in_to_refund: u128
            order_data.origin_chain_id, // origin_chain_id: u32,
        )?;

        emit_cpi!(OrderCancelled {
            order_id,
        });

        Ok(())
    }
}


#[event_cpi]
#[derive(Accounts)]
#[instruction(source_chain_id: u32, cancel_report: CancelReport)]
pub struct ReportOrderCancel<'info> {
    #[account(mut)]
    pub relayer: Signer<'info>,

    #[account(address = global_account.portal_authority @ OrderBookError::NotAuthorized)]
    pub portal_authority: Signer<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_account.bump
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(
        mut,
        seeds = [ORDER_SEED_PREFIX, cancel_report.order_id.as_ref()],
        bump = order.bump,
        constraint = order.data.dest_chain_id == source_chain_id @ OrderBookError::InvalidReportSource,
    )]
    pub order: Account<'info, Order::<NativeOrder>>,

    #[account(
        address = order.data.token_in @ OrderBookError::InvalidTokenMint,
        mint::token_program = token_in_program,
    )]
    pub token_in_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: This is validated against the stored order data
    #[account(
        address = order.data.sender @ OrderBookError::InvalidSender,
    )]
    pub order_sender: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = relayer,
        associated_token::mint = token_in_mint,
        associated_token::authority = order_sender,
        associated_token::token_program = token_in_program
    )]
    pub sender_token_in_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = token_in_mint,
        associated_token::authority = order.key(),
        associated_token::token_program = token_in_program
    )]
    pub order_token_in_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_in_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    pub system_program: Program<'info, System>,
}

impl ReportOrderCancel<'_> {
    fn validate(&self, cancel_report: &CancelReport) -> Result<()> {
        // Validate the order type is native
        require!(
            self.order.order_type == OrderType::Native,
            OrderBookError::InvalidOrderType
        );

        let status = &self.order.data.status;

        // Validate the order can be cancelled
        require!(
            status == &OrderStatus::Created,
            OrderBookError::InvalidOrderStatus
        );

        // Validate the reported order sender matches the stored order sender
        require_keys_eq!(
            Pubkey::new_from_array(cancel_report.order_sender),
            self.order.data.sender,
            OrderBookError::InvalidSender
        );

        // Validate the reported token_in matches the stored order token_in
        require_keys_eq!(
            Pubkey::new_from_array(cancel_report.token_in),
            self.order.data.token_in,
            OrderBookError::InvalidTokenMint
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&cancel_report))]
    pub fn handler(ctx: Context<Self>, _source_chain_id: u32, cancel_report: CancelReport) -> Result<()> {
        let order = &mut ctx.accounts.order.data;

        // Set the order status to Cancelled and increment the refunded amount with the reported amount
        order.status = OrderStatus::Cancelled;
        order.amount_in_refunded += cancel_report.amount_in_to_refund;

        require!(
            order.amount_in_released + order.amount_in_refunded <= order.amount_in,
            OrderBookError::InvalidRefundAmount
        );

        let amount_in_to_refund: u64 = cancel_report.amount_in_to_refund.try_into().map_err(|_| OrderBookError::MathOverflow)?;

        // Transfer the remaining inputs tokens back to the order sender
        // We do not check exact amount received here to avoid DoS refund / bridge message
        if amount_in_to_refund > 0 {
            transfer_tokens_from_program(
                &ctx.accounts.order_token_in_ata,
                &ctx.accounts.sender_token_in_ata,
                amount_in_to_refund,
                &ctx.accounts.token_in_mint,
                &ctx.accounts.order.to_account_info(),
                &[&[
                    ORDER_SEED_PREFIX,
                    &cancel_report.order_id,
                    &[ctx.accounts.order.bump],
                ]],
                &ctx.accounts.token_in_program,
            )?;
        } else {
            return err!(OrderBookError::OrderFilled);
        }

        emit_cpi!(RefundClaimed {
            order_id: cancel_report.order_id,
            sender: ctx.accounts.order_sender.key(),
            amount: amount_in_to_refund,
        });

        // Emit an event for the cancel report
        emit_cpi!(CancelReported {
            order_id: cancel_report.order_id,
        });

        Ok(())
    }
}

