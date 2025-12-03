use crate::{
    error::OrderBookError,
    state::{
        NativeOrder, Order, OrderBookGlobal, OrderStatus, OrderType, GLOBAL_SEED, ORDER_SEED_PREFIX,
    },
    utils::transfer_tokens_from_program,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct FillReport {
    pub order_id: [u8; 32],
    pub amount_in_to_release: u128,
    pub amount_out_filled: u128,
    pub origin_recipient: [u8; 32],
    pub token_in: [u8; 32],
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
            status == &OrderStatus::Created || status == &OrderStatus::CancelRequested,
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
        // We trust the amount provided is accurate and that the destination chain
        // does not allow overfills
        order.amount_in_released += fill_report.amount_in_to_release as u128;
        order.amount_out_filled += fill_report.amount_out_filled as u128;

        let full_fill = if order.amount_out_filled > order.amount_out
            || order.amount_in_released > order.amount_in
        {
            // This should not be possible, but included for safety
            return err!(OrderBookError::Overfill);
        } else if order.amount_out_filled == order.amount_out {
            // Mark the order as completed if fully filled
            order.status = OrderStatus::Completed;
            true
        } else {
            false
        };

        // Calculate the corresponding input amount to release to the solve
        // If the order is completed by the fill, use the order token account balance
        // Otherwise, use the reported amount
        let amount_in_to_release: u64 = if full_fill {
            // Any tokens sent to this account after the order is created are donated to the solver
            require!(
                ctx.accounts.order_token_in_ata.amount
                    >= fill_report
                        .amount_in_to_release
                        .try_into()
                        .map_err(|_| OrderBookError::MathOverflow)?,
                OrderBookError::InvalidFillAmount
            );
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

#[event]
pub struct FillReported {
    pub order_id: [u8; 32],
    pub amount_in_to_release: u128,
    pub amount_out_filled: u128,
    pub origin_recipient: [u8; 32],
}
