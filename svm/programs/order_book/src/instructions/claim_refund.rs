use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenInterface, TokenAccount};
use crate::{
    error::OrderBookError,
    state::{Order, NativeOrder, OrderStatus, ORDER_SEED_PREFIX, GLOBAL_SEED, OrderBookGlobal},
    utils::transfer_tokens_from_program,
};

#[event_cpi]
#[derive(Accounts)]
#[instruction(order_id: [u8; 32])]
pub struct ClaimRefund<'info> {
    /// CHECK: The sender of the order, we don't read any data from here
    /// This does not have to be a signer, anyone can claim refunds on behalf of the sender
    pub sender: UncheckedAccount<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_account.bump
    )]
    pub global_account: Account<'info, OrderBookGlobal>,
    
    #[account(
        mut,
        seeds = [ORDER_SEED_PREFIX, order_id.as_ref()],
        bump = order.bump,
        constraint = order.data.sender == sender.key() @ OrderBookError::NotAuthorized,
    )]
    pub order: Account<'info, Order::<NativeOrder>>,

    #[account(
        address = order.data.token_in @ OrderBookError::InvalidTokenMint,
        mint::token_program = token_in_program
    )]
    pub token_in_mint: InterfaceAccount<'info, Mint>,

    // TODO think about whether we should require this to be an ATA
    // If so, it may cause issues for programs that don't use ATAs
    // However, if we will allow anyone to claim refunds for senders,
    // then there may be some griefing risk
    #[account(
        mut,
        token::mint = token_in_mint,
        token::authority = sender,
        token::token_program = token_in_program,
    )]
    pub sender_token_in_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = token_in_mint,
        associated_token::authority = order,
        associated_token::token_program = token_in_program,
    )]
    pub order_token_in_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_in_program: Interface<'info, TokenInterface>,
}

impl ClaimRefund<'_> {
    fn validate(&self) -> Result<()> {
        let order = &self.order.data;

        // Validate the order has not been completed
        if order.status == OrderStatus::Completed {
            return err!(OrderBookError::InvalidOrderStatus);
        }

        // Validate the fill deadline has passed
        let current_timestamp = Clock::get()?.unix_timestamp as u64;
        if order.fill_deadline + self.global_account.finality_buffer > current_timestamp {
            return err!(OrderBookError::OrderNotExpired);
        }

        Ok(())
    }

    #[access_control(ctx.accounts.validate())]
    pub fn handler(ctx: Context<Self>, order_id: [u8; 32]) -> Result<()> {
        let order = &mut ctx.accounts.order.data;

        // Set the order status to Completed
        order.status = OrderStatus::Completed;

        // Transfer the remaining tokens in the order's token in ATA to the recipient
        let amount = ctx.accounts.order_token_in_ata.amount;
        if amount > 0 {
            transfer_tokens_from_program(
                &ctx.accounts.order_token_in_ata,
                &ctx.accounts.sender_token_in_account,
                amount,
                &ctx.accounts.token_in_mint,
                &ctx.accounts.order.to_account_info(),
                &[&[ORDER_SEED_PREFIX, order_id.as_ref(), &[ctx.accounts.order.bump]]],
                &ctx.accounts.token_in_program,
            )?;
        }

        emit_cpi!(RefundClaimed {
            order_id,
            amount,
        });

        Ok(())


    }
}

#[event]
pub struct RefundClaimed {
    pub order_id: [u8; 32],
    pub amount: u64,
}