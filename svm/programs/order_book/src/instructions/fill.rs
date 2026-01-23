use crate::{
    constants::{ANCHOR_DISCRIMINATOR_SIZE, VERSION}, 
    error::OrderBookError,
    state::{
        ForeignOrder, GLOBAL_SEED, NativeOrder, ORDER_SEED_PREFIX, Order, 
        OrderBookGlobal, OrderData, OrderStatus, OrderType, compute_order_id
    }, utils::{transfer_tokens_from_program, transfer_exact_tokens, transfer_exact_tokens_from_program}
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::portal::{
    constants::{AUTHORITY_SEED as PORTAL_AUTHORITY_SEED, GLOBAL_SEED as PORTAL_GLOBAL_SEED},
    cpi::{accounts::SendFillReport, send_fill_report},
    program::Portal,
    ID as PORTAL_ID
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
#[instruction(order_id: [u8; 32], order_data: Box<OrderData>, fill_params: FillParams)]
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
        address = Pubkey::new_from_array(order_data.token_out) @ OrderBookError::InvalidTokenOutMint,
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
        constraint = order.order_type == OrderType::Native @ OrderBookError::InvalidOrderType
    )]
    pub order: Account<'info, Order::<NativeOrder>>,

    #[account(
        mint::token_program = token_in_program,
        address = Pubkey::new_from_array(order_data.token_in) @ OrderBookError::InvalidTokenMint,
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
        order_data: Box<OrderData>,
        fill_params: FillParams,
    ) -> Result<()> {
        let order = &mut ctx.accounts.order.data;

        // Calculate the fill amount as the minimum of the provided fill amount out and the remaining amount out to fill
        // Also, calculate the corresponding amount in to release to the solver
        let (full_fill, amount_in_to_release, amount_out_to_fill) = calculate_fill(
            order_data.amount_in,
            order_data.amount_out,
            order.amount_in_released,
            order.amount_out_filled,
            fill_params.amount_out_to_fill as u128
        )?;

        if full_fill {
            // Set the order status to completed
            order.status = OrderStatus::Completed;
        }

        // Update the amount filled on the order
        order.amount_in_released += amount_in_to_release as u128;
        order.amount_out_filled += amount_out_to_fill as u128;

        // Transfer the output tokens from the solver to the recipient
        // Check that actual amount is received
        transfer_exact_tokens(
            &ctx.accounts.solver_token_out_account,
            &mut ctx.accounts.recipient_token_out_ata,
            amount_out_to_fill,
            &ctx.accounts.token_out_mint,
            &ctx.accounts.solver,
            &ctx.accounts.token_out_program,
        )?;

        // Transfer the input tokens from the order to the solver
        // Check that actual amount is received
        transfer_exact_tokens_from_program(
            &ctx.accounts.order_token_in_ata,
            &mut ctx.accounts.solver_token_in_account,
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

        // Prevent front-running: foreign order must not originate from this chain
        require!(order_data.origin_chain_id != self.global_account.chain_id,
            OrderBookError::InvalidOriginChainId
        );

        // Validate the origin recipient is not the zero address
        require!(
            fill_params.origin_recipient != [0u8; 32],
            OrderBookError::InvalidRecipient
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

        // Calculate the fill amount as the minimum of the provided fill amount out and the remaining amount out to fill
        // Also, calculate the corresponding amount in to release to the solver
        let (full_fill, amount_in_to_release, amount_out_to_fill) = calculate_fill(
            order_data.amount_in,
            order_data.amount_out,
            order.amount_in_released,
            order.amount_out_filled,
            fill_params.amount_out_to_fill as u128
        )?;

        if full_fill {
            // Set the order status to completed
            order.status = OrderStatus::Completed;
        };

        // Update the fill amounts on the order
        order.amount_in_released += amount_in_to_release as u128;
        order.amount_out_filled += amount_out_to_fill as u128;

        // Transfer the output tokens from the solver to the recipient
        // Check that actual amount is received
        transfer_exact_tokens(
            &ctx.accounts.solver_token_out_account,
            &mut ctx.accounts.recipient_token_out_ata,
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
#[instruction(source_chain_id: u32, fill_report: FillReport)]
pub struct ReportOrderFill<'info> {
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
        seeds = [ORDER_SEED_PREFIX, fill_report.order_id.as_ref()],
        bump = order.bump,
        constraint = order.data.dest_chain_id == source_chain_id @ OrderBookError::InvalidReportSource,
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

        // Validate the order can be filled.
        // Note: Fill reports are allowed on Cancelled orders because cross-chain messages
        // may arrive out of order. A fill that occurred before cancellation on the destination
        // chain might arrive after the cancel report on the origin chain. The amount tracking
        // (amount_in_released + amount_in_refunded <= amount_in) prevents over-distribution.
        require!(
            status == &OrderStatus::Created || status == &OrderStatus::Cancelled,
            OrderBookError::OrderNotFillable
        );

        // Validate the fill amount is not zero
        require!(
            fill_report.amount_out_filled > 0,
            OrderBookError::InvalidFillAmount
        );

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&fill_report))]
    pub fn handler(ctx: Context<Self>, _source_chain_id: u32, fill_report: FillReport) -> Result<()> {
        let order = &mut ctx.accounts.order.data;

        // Calculate expected fill amounts and compare to reported amounts
        let (full_fill, expected_amount_in_to_release, expected_amount_out_filled) = calculate_fill(
            order.amount_in,
            order.amount_out,
            order.amount_in_released,
            order.amount_out_filled,
            fill_report.amount_out_filled
        )?;

        // Check the filled amount out matches the report
        // It can be at most what was remaining to be filled
        require!(
            expected_amount_out_filled as u128 == fill_report.amount_out_filled,
            OrderBookError::InvalidFillAmount
        ); 
        // Check that the amount in to release matches the report
        // This confirms the ratio of the report matches the order
        require!(
            expected_amount_in_to_release as u128 == fill_report.amount_in_to_release,
            OrderBookError::InvalidFillAmount
        );

        // Update the filled amounts on the order
        order.amount_in_released += fill_report.amount_in_to_release;
        order.amount_out_filled += fill_report.amount_out_filled;

        // Validate the filled amounts do not exceed the order amounts
        // For tokenIn amounts, this includes both released and refunded amounts since
        // both reduce the amount available to be filled. Refunded amounts may have been
        // paid out previously via a cancel report.
        // We do not allow overfills.
        // Once an order is filled completely any excess token_in in the order_token_in_ata
        // (e.g. from a donation) can be claimed and the token account closed
        require!(
            order.amount_in_released + order.amount_in_refunded <= order.amount_in,
            OrderBookError::InvalidFillAmount
        );
        require!(
            order.amount_out_filled <= order.amount_out,
            OrderBookError::InvalidFillAmount
        );

        // Mark order as completed if fully filled
        if full_fill {
            // Mark the order as completed if fully filled
            order.status = OrderStatus::Completed;
            emit_cpi!(OrderCompleted { 
                order_id: fill_report.order_id
            });
        };

        // Transfer the input tokens from the order to the designated recipient
        // We do not check exact amount received here to avoid DoS on bridge message
        transfer_tokens_from_program(
            &ctx.accounts.order_token_in_ata,
            &ctx.accounts.recipient_token_in_ata,
            fill_report.amount_in_to_release as u64,
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

fn calculate_fill(
    total_amount_in_: u128,
    total_amount_out_: u128,
    amount_in_released_: u128,
    amount_out_filled_: u128,
    amount_out_to_fill_: u128
) -> Result<(bool, u64, u64)> {
    // Determine the amount out to fill as the minimum of the filler provided amount and the remaining unfilled amount
    let amount_out_remaining_ = total_amount_out_.checked_sub(amount_out_filled_).ok_or(OrderBookError::MathUnderflow)?;
    let full_fill_ = amount_out_to_fill_ >= amount_out_remaining_;
    let amount_out_to_fill_ = if full_fill_ {
        amount_out_remaining_
    } else {
        amount_out_to_fill_
    };

    // Calculate the corresponding amount of token in to release to the filler
    let amount_in_to_release_ = if full_fill_ {
        total_amount_in_.checked_sub(amount_in_released_).ok_or(OrderBookError::MathUnderflow)? // remaining amount
    } else {
        total_amount_in_.checked_mul(amount_out_to_fill_).ok_or(OrderBookError::MathOverflow)?
            .checked_div(total_amount_out_).ok_or(OrderBookError::MathUnderflow)?
    };

    Ok((
        full_fill_, 
        amount_in_to_release_.try_into().map_err(|_| OrderBookError::InvalidFillAmount)?, 
        amount_out_to_fill_.try_into().map_err(|_| OrderBookError::InvalidFillAmount)?
    ))
}