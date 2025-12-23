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
pub struct CancelReport {
    pub order_id: [u8; 32],
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(cancel_report: CancelReport)]
pub struct ReportOrderCancel<'info> {
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

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&fill_report))]
    pub fn handler(ctx: Context<Self>, fill_report: FillReport) -> Result<()> {
        let order = &mut ctx.accounts.order.data;

        // Set the order status to Cancelled
        order.status = OrderStatus::Cancelled;
        

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


