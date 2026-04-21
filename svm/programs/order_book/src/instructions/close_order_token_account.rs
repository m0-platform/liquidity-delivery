use crate::{
    error::OrderBookError,
    state::{NativeOrder, Order, OrderStatus, ORDER_SEED_PREFIX},
    utils::transfer_tokens_from_program,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    close_account, CloseAccount, Mint, TokenAccount, TokenInterface,
};

/// Close the order's token account and return SOL rent to the original payer
///
/// This instruction:
/// - Closes the order_token_in_ata (transferring any dust tokens to sender first)
/// - Returns the ~0.002 SOL rent to the payer who originally created the order
/// - Can be called by anyone (permissionless) after the order is finalized
///
/// Note: This only reclaims SOL rent. SPL tokens are already returned via
/// the cancel/fill instructions. If there are any dust tokens remaining (e.g. from
/// griefing donations), they are transferred to the sender's token account first.
#[derive(Accounts)]
#[instruction(order_id: [u8; 32])]
pub struct CloseOrderTokenAccount<'info> {
    /// The original payer who paid SOL rent for the ATA
    /// SOL rent will be refunded to this account
    /// CHECK: Validated against order.data.payer in the validation function
    #[account(  
        mut,  
        address = order.data.payer @ OrderBookError::InvalidPayer,  
    )]
    pub payer: UncheckedAccount<'info>,

    /// CHECK: The sender of the order, validated against order.data.sender
    #[account(
        mut,
        address = order.data.sender @ OrderBookError::InvalidSender
    )]
    pub sender: UncheckedAccount<'info>,

    /// The order account - must be in Completed or Cancelled status
    #[account(
        seeds = [ORDER_SEED_PREFIX, &order_id],
        bump = order.bump,
    )]
    pub order: Box<Account<'info, Order::<NativeOrder>>>,

    /// The token mint for validation
    #[account(
        address = order.data.token_in @ OrderBookError::InvalidTokenMint,
        mint::token_program = token_in_program
    )]
    pub token_in_mint: InterfaceAccount<'info, Mint>,

    /// The order's ATA to close
    #[account(
        mut,
        associated_token::mint = token_in_mint,
        associated_token::authority = order,
        associated_token::token_program = token_in_program,
    )]
    pub order_token_in_ata: InterfaceAccount<'info, TokenAccount>,

    /// Optional recipient for any dust tokens (due to griefing donations)
    /// Must be owned by the order sender
    #[account(
        mut,
        token::mint = token_in_mint,
        token::authority = sender,
        token::token_program = token_in_program,
    )]
    pub recipient_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    /// SPL Token or Token-2022 program
    pub token_in_program: Interface<'info, TokenInterface>,
}

impl CloseOrderTokenAccount<'_> {
    fn validate(&self) -> Result<()> {
        let order = &self.order.data;

        // Verify order is in a finalized state (Completed or Cancelled)
        // For cancelled orders, we need to ensure all funds have been released/refunded
        // so that amounts reserved for fills that have not yet been reported cannot
        // be swept as dust.
        require!(
            order.status == OrderStatus::Completed || (
                order.status == OrderStatus::Cancelled && order.amount_in_released + order.amount_in_refunded == order.amount_in
            ),
            OrderBookError::InvalidOrderStatus
        );

        // If token account has a non-zero balance (e.g. from griefing donations),
        // require that a recipient token account is provided to receive the dust
        if self.order_token_in_ata.amount > 0 {
            require!(
                self.recipient_token_account.is_some(),
                OrderBookError::DustRecipientRequired
            );
        }

        Ok(())
    }

    #[access_control(ctx.accounts.validate())]
    pub fn handler(ctx: Context<Self>, order_id: [u8; 32]) -> Result<()> {
        // Build PDA signer seeds
        let order_seeds: &[&[&[u8]]] = &[&[
            ORDER_SEED_PREFIX,
            order_id.as_ref(),
            &[ctx.accounts.order.bump],
        ]];

        // If there are any dust tokens (from griefing donations), transfer them to sender first
        let dust_amount = ctx.accounts.order_token_in_ata.amount;
        if dust_amount > 0 {
            let recipient = ctx.accounts.recipient_token_account.as_ref().unwrap();

            transfer_tokens_from_program(
                &ctx.accounts.order_token_in_ata,
                recipient,
                dust_amount,
                &ctx.accounts.token_in_mint,
                &ctx.accounts.order.to_account_info(),
                order_seeds,
                &ctx.accounts.token_in_program,
            )?;
        }

        // Close the ATA and return SOL rent to payer
        close_account(CpiContext::new_with_signer(
            ctx.accounts.token_in_program.to_account_info(),
            CloseAccount {
                account: ctx.accounts.order_token_in_ata.to_account_info(),
                destination: ctx.accounts.payer.to_account_info(),
                authority: ctx.accounts.order.to_account_info(),
            },
            order_seeds,
        ))?;

        Ok(())
    }
}
