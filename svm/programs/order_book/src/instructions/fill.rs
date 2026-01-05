use crate::{
    constants::{ANCHOR_DISCRIMINATOR_SIZE, VERSION}, 
    error::OrderBookError,
    state::{
        ForeignOrder, GLOBAL_SEED, NativeOrder, ORDER_SEED_PREFIX, Order, 
        OrderBookGlobal, OrderData, OrderStatus, OrderType, compute_order_id
    }, utils::{transfer_tokens, transfer_tokens_from_program}
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::portal::{
    cpi::{accounts::SendFillReport, send_fill_report},
    program::Portal,
};

// Instructions related to filling orders
// Orders must be filled on their destination chain.
// From the perspective of the chain that this program is deployed on,
// there are two main flows:
// 1. filling a samechain order (i.e. current chain ID == origin chain ID == destination chain ID)
//   a. for orders that both originate on the current chain
//      and have the current chain as the destination
//      the designated solver (if provided) or anyone (if no solver specified)
//      fills the order by executing `FillNativeOrder`
// 2. filling a cross-chain order
//   a. for orders that have the current chain as the destination, (i.e. current chain ID == destination chain ID != origin chain ID)
//      the designated solver (if provided) or anyone (if no solver specified)
//      fills the order by executing `FillForeignOrder`
//      this sends a cancel report back to the origin chain via a CPI to the Portal program
//   b. for orders that originate on the current chain, (i.e. current chain ID == origin chain ID != destination chain ID)
//      the relayer reports the fills back to the origin chain by executing `ReportOrderFill`
//      via the Portal program

// Handler Inputs
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct FillParams {
    pub amount_out_to_fill: u64,
    pub origin_recipient: [u8; 32],
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct FillReport {
    pub order_id: [u8; 32],
    pub amount_in_to_release: u128,
    pub amount_out_filled: u128,
    pub origin_recipient: [u8; 32],
    pub token_in: [u8; 32],
}

// Events
#[event]
pub struct OrderFilled {
    pub order_id: [u8; 32],
    pub solver: Pubkey,
    pub amount_in_to_release: u128,
    pub amount_out_filled: u128,
}

#[event]
pub struct OrderCompleted {
    pub order_id: [u8; 32],
}

#[event]
pub struct FillReported {
    pub order_id: [u8; 32],
    pub amount_in_to_release: u128,
    pub amount_out_filled: u128,
    pub origin_recipient: [u8; 32],
}

// Instruction Contexts and Handlers
#[event_cpi]
#[derive(Accounts)]
#[instruction(order_id: [u8; 32], order_data: OrderData, fill_params: FillParams)]
pub struct FillNativeOrder<'info> {
    #[account(mut)]
    pub solver: Signer<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_account.bump,
        constraint = order_data.dest_chain_id == global_account.chain_id @ OrderBookError::InvalidDestChainId,
        constraint = !global_account.paused @ OrderBookError::ProgramPaused,
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(
        mint::token_program = token_out_program,
        constraint = token_out_mint.key().to_bytes() == order_data.token_out @ OrderBookError::InvalidTokenOutMint,
    )]
    pub token_out_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = token_out_mint,
        token::token_program = token_out_program,
    )]
    pub solver_token_out_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: This is validated against the order data
    #[account(
        address = Pubkey::new_from_array(order_data.recipient) @ OrderBookError::InvalidRecipient,
    )]
    pub recipient: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = solver,
        associated_token::mint = token_out_mint,
        associated_token::authority = recipient,
        associated_token::token_program = token_out_program,
    )]
    pub recipient_token_out_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_out_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    pub system_program: Program<'info, System>,

    #[account(
        mut,
        seeds = [ORDER_SEED_PREFIX, &order_id],
        bump = order.bump,
    )]
    pub order: Account<'info, Order::<NativeOrder>>,

    #[account(
        mint::token_program = token_in_program,
    )]
    pub token_in_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = token_in_mint,
        token::authority = Pubkey::new_from_array(fill_params.origin_recipient),
        token::token_program = token_in_program,
    )]
    pub solver_token_in_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = token_in_mint,
        associated_token::authority = order,
        associated_token::token_program = token_in_program,
    )]
    pub order_token_in_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_in_program: Interface<'info, TokenInterface>,
}

impl FillNativeOrder<'_> {
    fn validate(
        &self,
        order_id: &[u8; 32],
        order_data: &OrderData,
        fill_params: &FillParams,
    ) -> Result<()> {
        // Validate the params
        validate_params(order_id, order_data, fill_params, &self.solver.key())?;

        // Validate the origin chain ID is this chain
        require!(
            order_data.origin_chain_id == self.global_account.chain_id,
            OrderBookError::InvalidOriginChainId
        );

        // Validate the order is in a fillable state
        require!(
            self.order.data.status == OrderStatus::Created,
            OrderBookError::OrderNotFillable
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&order_id, &order_data, &fill_params))]
    pub fn handler(
        ctx: Context<Self>,
        order_id: [u8; 32],
        order_data: OrderData,
        fill_params: FillParams,
    ) -> Result<()> {
        let order = &mut ctx.accounts.order.data;

        // Calculate the fill amount as the minimum of the provided fill amount out and the remaining amount out to fill
        // Also, calculate the corresponding amount in to release to the solver
        let amount_out_remaining: u128 = order
            .amount_out
            .checked_sub(order.amount_out_filled)
            .ok_or(OrderBookError::MathUnderflow)?;
        let amount_in_remaining: u128 = order
            .amount_in
            .checked_sub(order.amount_in_released)
            .ok_or(OrderBookError::MathUnderflow)?;
        require!(amount_out_remaining > 0, OrderBookError::OrderFilled);
        let full_fill: bool = fill_params.amount_out_to_fill as u128 >= amount_out_remaining;
        let (amount_in_to_release, amount_out_to_fill): (u64, u64) = if full_fill {
            // Set the order status to completed
            order.status = OrderStatus::Completed;

            // Set the fill amount out to the remaining amount
            // The amount in to release is the remaining amount in the order ATA
            // Any extra tokens are considered a donation to the solver that completes the order
            require!(
                ctx.accounts.order_token_in_ata.amount
                    >= amount_in_remaining
                        .try_into()
                        .map_err(|_| OrderBookError::InvalidFillAmount)?,
                OrderBookError::InvalidFillAmount
            );
            (
                ctx.accounts.order_token_in_ata.amount,
                amount_out_remaining
                    .try_into()
                    .map_err(|_| OrderBookError::InvalidFillAmount)?,
            )
        } else {
            // Calculate the amount in to release based on the proportion of amount out being filled
            let amount_in_to_release: u64 = (fill_params.amount_out_to_fill as u128)
                .checked_mul(order.amount_in)
                .ok_or(OrderBookError::MathOverflow)?
                .checked_div(order.amount_out)
                .ok_or(OrderBookError::MathUnderflow)?
                .try_into()
                .map_err(|_| OrderBookError::MathOverflow)?;

            (amount_in_to_release, fill_params.amount_out_to_fill)
        };

        // Update the amount filled on the order
        order.amount_in_released += amount_in_to_release as u128;
        order.amount_out_filled += amount_out_to_fill as u128;

        // Transfer the output tokens from the solver to the recipient
        transfer_tokens(
            &ctx.accounts.solver_token_out_account,
            &ctx.accounts.recipient_token_out_ata,
            amount_out_to_fill,
            &ctx.accounts.token_out_mint,
            &ctx.accounts.solver,
            &ctx.accounts.token_out_program,
        )?;

        // Transfer the input tokens from the order to the solver
        transfer_tokens_from_program(
            &ctx.accounts.order_token_in_ata,
            &ctx.accounts.solver_token_in_account,
            amount_in_to_release,
            &ctx.accounts.token_in_mint,
            &ctx.accounts.order.to_account_info(),
            &[&[ORDER_SEED_PREFIX, &order_id, &[ctx.accounts.order.bump]]],
            &ctx.accounts.token_in_program,
        )?;

        // Emit a fill event regardless
        emit_cpi!(OrderFilled {
            order_id,
            solver: ctx.accounts.solver.key(),
            amount_in_to_release: amount_in_to_release as u128,
            amount_out_filled: amount_out_to_fill as u128,
        });

        // If the order is fully filled, emit an order completed event
        if full_fill {
            emit_cpi!(OrderCompleted { order_id });
        }

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(order_id: [u8; 32], order_data: OrderData)]
pub struct FillForeignOrder<'info> {
    #[account(mut)]
    pub solver: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = global_account.bump,
        constraint = order_data.dest_chain_id == global_account.chain_id @ OrderBookError::InvalidDestChainId,
        constraint = !global_account.paused @ OrderBookError::ProgramPaused,
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(
        mint::token_program = token_out_program,
        constraint = token_out_mint.key().to_bytes() == order_data.token_out @ OrderBookError::InvalidTokenOutMint,
    )]
    pub token_out_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = token_out_mint,
        token::token_program = token_out_program,
    )]
    pub solver_token_out_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: This is validated against the order data
    #[account(
        address = Pubkey::new_from_array(order_data.recipient) @ OrderBookError::InvalidRecipient,
    )]
    pub recipient: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = solver,
        associated_token::mint = token_out_mint,
        associated_token::authority = recipient,
        associated_token::token_program = token_out_program,
    )]
    pub recipient_token_out_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_out_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    pub system_program: Program<'info, System>,

    #[account(
        init_if_needed,
        payer = solver,
        space = ANCHOR_DISCRIMINATOR_SIZE + Order::<ForeignOrder>::INIT_SPACE,
        seeds = [ORDER_SEED_PREFIX, &order_id],
        bump
    )]
    pub order: Account<'info, Order::<ForeignOrder>>,

    pub portal_program: Program<'info, Portal>,

    /// CHECK: Portal global account
    /// This is validated in the portal CPI
    #[account(mut)]
    pub portal_global: UncheckedAccount<'info>,

    /// CHECK: Portal authority PDA
    /// This is validated in the portal CPI
    pub portal_authority: UncheckedAccount<'info>,

    /// CHECK: Bridge adapter program
    /// This is validated in the portal CPI
    pub bridge_adapter: UncheckedAccount<'info>,
}

impl<'info> FillForeignOrder<'info> {
    fn validate(
        &self,
        order_id: &[u8; 32],
        order_data: &OrderData,
        fill_params: &FillParams,
    ) -> Result<()> {
        // Validate the params
        validate_params(order_id, order_data, fill_params, &self.solver.key())?;

        // Validate the order status is fillable (i.e. DoesNotExist or Created, if already partially filled)
        require!(
            self.order.data.status == OrderStatus::DoesNotExist || self.order.data.status == OrderStatus::Created,
            OrderBookError::OrderNotFillable
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&order_id, &order_data, &fill_params))]
    pub fn handler(
        ctx: Context<'_, '_, 'info, 'info, Self>,
        order_id: [u8; 32],
        order_data: OrderData,
        fill_params: FillParams,
    ) -> Result<()> {
        // If this is a new order, initialize it
        if ctx.accounts.order.data.status == OrderStatus::DoesNotExist {
            ctx.accounts.order.order_type = OrderType::Foreign;
            ctx.accounts.order.bump = ctx.bumps.order;
            ctx.accounts.order.data = ForeignOrder {
                status: OrderStatus::Created,
                amount_in_released: 0,
                amount_out_filled: 0,
            };
        }

        let order = &mut ctx.accounts.order.data;

        // Calculate the fill amount as the minimum of the provided fill amount out and the remaining amount out to fill
        // Also, calculate the corresponding amount in to release to the solver
        let amount_out_remaining: u128 = order_data
            .amount_out
            .checked_sub(order.amount_out_filled)
            .ok_or(OrderBookError::MathUnderflow)?;
        let amount_in_remaining: u128 = order_data
            .amount_in
            .checked_sub(order.amount_in_released)
            .ok_or(OrderBookError::MathUnderflow)?;
        require!(amount_out_remaining > 0, OrderBookError::OrderFilled);
        let full_fill: bool = fill_params.amount_out_to_fill as u128 >= amount_out_remaining;
        let (amount_in_to_release, amount_out_to_fill): (u64, u64) = if full_fill {
            // Set the order status to completed
            order.status = OrderStatus::Completed;

            // Set the fill amount out to the remaining amount
            // Set the amount in to release to the remaining amount in
            (
                amount_in_remaining
                    .try_into()
                    .map_err(|_| OrderBookError::InvalidFillAmount)?,
                amount_out_remaining
                    .try_into()
                    .map_err(|_| OrderBookError::InvalidFillAmount)?,
            )
        } else {
            // Calculate the amount in to release based on the proportion of amount out being filled
            let amount_in_to_release: u64 = (fill_params.amount_out_to_fill as u128)
                .checked_mul(order_data.amount_in)
                .ok_or(OrderBookError::MathOverflow)?
                .checked_div(order_data.amount_out)
                .ok_or(OrderBookError::MathUnderflow)?
                .try_into()
                .map_err(|_| OrderBookError::MathOverflow)?;

            (amount_in_to_release, fill_params.amount_out_to_fill)
        };

        // Update the fill amounts on the order
        order.amount_in_released += amount_in_to_release as u128;
        order.amount_out_filled += amount_out_to_fill as u128;

        // Transfer the output tokens from the solver to the recipient
        transfer_tokens(
            &ctx.accounts.solver_token_out_account,
            &ctx.accounts.recipient_token_out_ata,
            amount_out_to_fill,
            &ctx.accounts.token_out_mint,
            &ctx.accounts.solver,
            &ctx.accounts.token_out_program,
        )?;

        // Send a fill report message to the origin chain via the portal program
        send_fill_report(
            CpiContext::new_with_signer(
                ctx.accounts.portal_program.to_account_info(),
                SendFillReport {
                    sender: ctx.accounts.solver.to_account_info(),
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
            order_data.token_in, // token_in: [u8; 32],
            amount_in_to_release as u128, // amount_in_to_release: u128,
            amount_out_to_fill as u128, // amount_out_filled: u128,
            fill_params.origin_recipient, // origin_recipient: [u8; 32],
            order_data.origin_chain_id, // origin_chain_id: u32,
        )?;

        // Emit a fill event
        emit_cpi!(OrderFilled {
            order_id,
            solver: ctx.accounts.solver.key(),
            amount_in_to_release: amount_in_to_release as u128,
            amount_out_filled: amount_out_to_fill as u128,
        });

        Ok(())
    }
}

fn validate_params(
    order_id: &[u8; 32],
    order_data: &OrderData,
    fill_params: &FillParams,
    solver_account_key: &Pubkey,
) -> Result<()> {
    // Validate the provided order ID matches the order data
    // We allow passing this in as a sanity check for callers
    // This also means we don't need to check the order data against the onchain data
    let computed_order_id = compute_order_id(order_data);
    require!(
        computed_order_id == *order_id,
        OrderBookError::InvalidOrderId
    );

    let current_timestamp = Clock::get()?.unix_timestamp as u64;
    // Validate the order has not expired
    require!(
        current_timestamp <= order_data.fill_deadline as u64,
        OrderBookError::OrderExpired
    );

    // Validate the created_at timestamp is not in the future
    require!(
        current_timestamp >= order_data.created_at,
        OrderBookError::InvalidCreatedAtTimestamp
    );

    // Validate the order is for the current version
    require!(
        order_data.version == VERSION,
        OrderBookError::InvalidOrderVersion
    );

    // Validate the fill amount is not zero
    require!(
        fill_params.amount_out_to_fill > 0,
        OrderBookError::InvalidFillAmount
    );

    // If the order solver is populated (i.e. not all zeros), validate it matches the signer
    if order_data.solver != [0u8; 32] {
        let solver_pubkey = Pubkey::new_from_array(order_data.solver);
        require!(
            solver_account_key.eq(&solver_pubkey),
            OrderBookError::InvalidSolver
        );
    }

    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(fill_report: FillReport)]
pub struct ReportOrderFill<'info> {
    #[account(mut)]
    pub relayer: Signer<'info>,

    #[account(address = global_account.messenger_authority @ OrderBookError::NotAuthorized)]
    pub messenger_authority: Signer<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_account.bump
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(
        mut,
        seeds = [ORDER_SEED_PREFIX, fill_report.order_id.as_ref()],
        bump = order.bump,
    )]
    pub order: Account<'info, Order::<NativeOrder>>,

    #[account(
        address = order.data.token_in @ OrderBookError::InvalidTokenMint,
        mint::token_program = token_in_program,
    )]
    pub token_in_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: This is validated against the fill report
    #[account(
        address = Pubkey::new_from_array(fill_report.origin_recipient) @ OrderBookError::InvalidRecipient,
    )]
    pub origin_recipient: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = relayer,
        associated_token::mint = token_in_mint,
        associated_token::authority = origin_recipient,
        associated_token::token_program = token_in_program
    )]
    pub recipient_token_in_ata: InterfaceAccount<'info, TokenAccount>,

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

impl ReportOrderFill<'_> {
    fn validate(&self, fill_report: &FillReport) -> Result<()> {
        // Validate the order type is native
        require!(
            self.order.order_type == OrderType::Native,
            OrderBookError::InvalidOrderType
        );

        let status = &self.order.data.status;

        // Validate the order can be filled
        require!(
            status == &OrderStatus::Created,
            OrderBookError::OrderNotFillable
        );

        // Validate the fill amount is not zero
        if fill_report.amount_out_filled == 0 {
            return err!(OrderBookError::InvalidFillAmount);
        }

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&fill_report))]
    pub fn handler(ctx: Context<Self>, fill_report: FillReport) -> Result<()> {
        let order = &mut ctx.accounts.order.data;

        // Update the filled amounts on the order
        order.amount_in_released += fill_report.amount_in_to_release as u128;
        order.amount_out_filled += fill_report.amount_out_filled as u128;

        let full_fill = if order.amount_out_filled >= order.amount_out {
            // The amount_in_to_release is limited to the amount in the order tokenIn ATA
            // Therefore, we don't need to check for overfills here.

            // Mark the order as completed if fully filled
            order.status = OrderStatus::Completed;
            true
        } else {
            false
        };

        // Calculate the corresponding input amount to release to the solve
        // If the order is completed by the fill, use the order token account balance
        // Otherwise, use the reported amount
        // If the reported amount is more than the order token account balance, it will error during the transfer.
        // This shouldn't happen in normal operation and if it does, then the transfer error will stop the ix
        // from completing.
        let amount_in_to_release: u64 = if full_fill {
            // Any tokens sent to this account after the order is created are donated to the solver
            ctx.accounts.order_token_in_ata.amount
        } else {
            fill_report
                .amount_in_to_release
                .try_into()
                .map_err(|_| OrderBookError::MathOverflow)?
        };

        // Transfer the input tokens from the order to the designated recipient
        transfer_tokens_from_program(
            &ctx.accounts.order_token_in_ata,
            &ctx.accounts.recipient_token_in_ata,
            amount_in_to_release,
            &ctx.accounts.token_in_mint,
            &ctx.accounts.order.to_account_info(),
            &[&[
                ORDER_SEED_PREFIX,
                &fill_report.order_id,
                &[ctx.accounts.order.bump],
            ]],
            &ctx.accounts.token_in_program,
        )?;

        // Emit an event for the fill report
        emit_cpi!(FillReported {
            order_id: fill_report.order_id,
            amount_in_to_release: fill_report.amount_in_to_release,
            amount_out_filled: fill_report.amount_out_filled,
            origin_recipient: fill_report.origin_recipient,
        });

        Ok(())
    }
}



