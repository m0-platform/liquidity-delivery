use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

pub struct FillParams {
    pub amount_out_to_fill: u64,
    pub origin_recipient: [u8; 32],
}

#[derive(Accounts)]
#[instruction(order_id: [u8; 32], order_data: OrderData)]
pub struct FillCommon<'info> {
    #[account(mut)]
    pub solver: Signer<'info>,

    pub token_authority: Option<Signer<'info>>,

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

    #[account(
        init_if_needed,
        associated_token::mint = token_out_mint,
        associated_token::authority = Pubkey::new_from_array(order_data.recipient),
        associated_token::token_program = token_out_program,
    )]
    pub recipient_token_out_ata: InterfaceAccount<'info, TokenAccount>,
    
    pub token_out_program: Interface<'info, TokenInterface>,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(order_id: [u8; 32], order_data: OrderData)]
pub struct FillNativeOrder<'info> {
    pub common: FillCommon<'info>,

    #[account(
        mut,
        seeds = [ORDER_SEED_PREFIX, &order_id],
        bump = order.bump,
        constraint = order.order_type == OrderType::Native @ OrderBookError::InvalidOrderType,
    )]
    pub order: Account<'info, Order<NativeOrder>>,

    #[account(
        mint::token_program = token_in_program,
    )]
    pub token_in_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = token_in_mint,
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
    fn validate(&self, order_id: &[u8; 32], order_data: &OrderData, fill_params: &FillParams) -> Result<()> {
        // Validate the params
        validate_params(order_id, order_data, fill_params, &self.common.solver.key())?;

        // Validate the order is in a fillable state
        require!(self.order.data.status == OrderStatus::Created, OrderBookError::OrderNotFillable);

        Ok(())
    }

    #[access_control(self.validate(&order_id, &order_data, &fill_params))]
    pub fn handler(Context<Self>, order_id: [u8; 32], order_data: OrderData, fill_params: FillParams) -> Result<()> {
        let order = &mut ctx.accounts.order.data;

        // Calculate the fill amount as the minimum of the provided fill amount out and the remaining amount out to fill
        let outFilled: u128 = order.amount_out_filled;
        let outRemaining: u128 = order.amount_out.checked_sub(outFilled).ok_or(OrderBookError::MathOverflow)?;
        let full_fill: bool = fill_params.amount_out_to_fill as u128 >= outRemaining;
        let fill_amount_out: u64 = if full_fill {
            // Since the order is fully filled, update the order status
            order.status = OrderStatus::Completed;

            // Set the fill amount to the remaining amount
            outRemaining.try_into().map_err(|_| OrderBookError::InvalidFillAmount)?
        } else {
            // Otherwise, just use the provided fill amount
            fill_params.amount_out_to_fill
        };

        // Update the amount filled on the order
        order.amount_out_filled += fill_amount_out as u128;

        // Calculate the corresponding input amount to release to the solve
        // If the order is completed by the fill, use the order token account balance
        // Otherwise, calculate pro-rata based on the fill amount
        let release_amount_in: u64 = if full_fill {
            // Any tokens sent to this account after the order is created are donated to the solver
            ctx.accounts.order.token_in_account.amount
        } else {
            (fill_amount_out as u128)
                .checked_mul(order.amount_in as u128).ok_or(OrderBookError::MathOverflow)?
                .checked_div(order.amount_out).ok_or(OrderBookError::MathOverflow)?
                .try_into().map_err(|_| OrderBookError::MathOverflow)?
        };

        // Transfer the output tokens from the solver to the recipient
        let auth = match ctx.accounts.common.token_authority {
            Some(signer) => signer.to_account_info(),
            None => ctx.accounts.common.solver.to_account_info(),
        };

        transfer_tokens(
            &ctx.accounts.common.solver_token_out_account,
            &ctx.accounts.common.recipient_token_out_ata,
            fill_amount_out,
            &ctx.accounts.common.token_out_mint,
            &auth,
            &ctx.accounts.common.token_out_program,
        )?;

        // Transfer the input tokens from the order to the solver
        let auth_seeds = &[&[
            ORDER_SEED_PREFIX,
            &order_id,
            &[ctx.accounts.order.bump],
        ]];

        transfer_tokens_from_program(
            &ctx.accounts.order.token_in_account,
            &ctx.accounts.common.solver_token_in_ata,
            release_amount_in,
            &ctx.accounts.common.token_in_mint,
            &ctx.accounts.order.to_account_info(),
            &auth_seeds,
            &ctx.accounts.common.token_in_program,
        )?;

                
        // If the order is fully filled, emit an order completed event
        if full_fill {
            emit_cpi!(
                OrderCompleted {
                    order_id,
                }
            );
        }

        // Emit a fill event regardless
        emit_cpi!(
            Fill {
                order_id,
                solver: ctx.accounts.common.solver.key(),
                amount_out_filled: fill_amount_out,
            }
        );
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(order_id: [u8; 32], order_data: OrderData)]
pub struct FillForeignOrder<'info> {
    pub common: FillCommon<'info>,

    #[account(
        init_if_needed,
        payer = common.solver,
        space = ANCHOR_DISCRIMINATOR_SIZE + Order::<ForeignOrder>::INIT_SPACE,
        seeds = [ORDER_SEED_PREFIX, &order_id],
        bump = order.bump,
    )]
    pub order: Account<'info, Order<ForeignOrder>>,

    /// CHECK: TODO - need to determine the program that we will use to send the fill report messages
    pub messenger_program: AccountInfo<'info>,
}

impl FillForeignOrder<'_> {

    fn validate(self, order_id: &[u8; 32], order_data: &OrderData, fill_params: &FillParams) -> Result<()> {
        // Validate the params
        validate_params(order_id, order_data, fill_params, &self.common.solver.key())?;

        // Validate the order is in a fillable state
        require!(self.order.amount_out_filled < order_data.amount_out, OrderBookError::OrderNotFillable);

        Ok(())
    }

    #[access_control(self.validate(&order_id, &order_data, &fill_params))]
    pub fn handler(Context<Self>, order_id: [u8; 32], order_data: OrderData, fill_params: FillParams) -> Result<()> {
        let order = &mut ctx.accounts.order;

        // If this is a new order, initialize it
        if order.amount_out_filled == 0 {
            ctx.accounts.order.order_type = OrderType::Foreign;
            ctx.accounts.order.bump = ctx.bumps.order;
        }

        // Calculate the fill amount as the minimum of the provided fill amount out and the remaining amount out to fill
        let outFilled: u128 = order.amount_out_filled;
        let outRemaining: u128 = order_data.amount_out.checked_sub(outFilled).ok_or(OrderBookError::MathOverflow)?;
        let full_fill: bool = fill_params.amount_out_to_fill as u128 >= outRemaining;
        let fill_amount_out: u64 = if full_fill {
            // Set the fill amount to the remaining amount
            outRemaining.try_into().map_err(|_| OrderBookError::InvalidFillAmount)?
        } else {
            // Otherwise, just use the provided fill amount
            fill_params.amount_out_to_fill
        };

        // Update the amount filled on the order
        order.amount_out_filled += fill_amount_out as u128;

        // Transfer the output tokens from the solver to the recipient
        let auth = match ctx.accounts.common.token_authority {
            Some(signer) => signer.to_account_info(),
            None => ctx.accounts.common.solver.to_account_info(),
        };

        transfer_tokens(
            &ctx.accounts.common.solver_token_out_account,
            &ctx.accounts.common.recipient_token_out_ata,
            fill_amount_out,
            &ctx.accounts.common.token_out_mint,
            &auth,
            &ctx.accounts.common.token_out_program,
        )?;

        // Send a fill report message to the origin chain via the messenger program
        // TODO: implement

        // Emit a fill event
        emit_cpi!(
            Fill {
                order_id,
                solver: ctx.accounts.common.solver.key(),
                amount_out_filled: fill_amount_out,
            }
        );
    }

}

fn validate_params(order_id: &[u8; 32], order_data: &OrderData, fill_params: &FillParams, solver_account_key: &Pubkey) -> Result<()> {
    // Validate the provided order ID matches the order data
    // We allow passing this in as a sanity check for callers
    // This also means we don't need to check the order data against the onchain data
    let computed_order_id = compute_order_id(*order_data);
    require!(computed_order_id == *order_id, OrderBookError::InvalidOrderId);

    // Validate the order has not expired
    let current_timestamp = Clock::get()?.unix_timestamp as u64;
    require!(current_timestamp <= order_data.fill_deadline, OrderBookError::OrderExpired);

    // Validate the order is for the current version
    require!(order_data.version == VERSION, OrderBookError::InvalidOrderVersion);

    // Validate the destination chain ID matches the current chain
    require!(order_data.dest_chain_id == CHAIN_ID, OrderBookError::InvalidDestChainId);

    // Validate the fill amount is not zero
    require!(fill_params.amount_out_to_fill > 0, OrderBookError::InvalidFillAmount);

    // Validate the origin recipient is a valid pubkey
    let _ = Pubkey::new_from_array(fill_params.origin_recipient);

    // If the order solver is populated (i.e. not all zeros), validate it matches the signer
    if order_data.solver != [0u8; 32] {
        let solver_pubkey = Pubkey::new_from_array(order_data.solver);
        require!(self.common.solver.key() == solver_pubkey, OrderBookError::InvalidSolver);
    }

    Ok(()) 
}