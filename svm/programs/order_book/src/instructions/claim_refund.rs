use crate::{
    ID,
    error::OrderBookError,
    state::{
        Destination, NativeOrder, Order, OrderBookGlobal, OrderStatus, DESTINATION_SEED_PREFIX,
        GLOBAL_SEED, ORDER_SEED_PREFIX,
    },
    utils::transfer_tokens_from_program,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

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

    // #[account(
    //     seeds = [DESTINATION_SEED_PREFIX, order.data.dest_chain_id.to_be_bytes().as_ref()],
    //     bump = destination_account.bump
    // )]
    /// We validate the seed in the validate() function to avoid issues
    /// with IDL generation caused by using the value nested with Order::<NativeOrder>
    pub destination_account: Option<Account<'info, Destination>>,

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

impl ClaimRefund<'_> {
    fn validate(&self) -> Result<()> {
        // Validate the destination account exists if the order's destination chain is not the current chain
        let finality_buffer = if self.order.data.dest_chain_id != self.global_account.chain_id {
            // Validate the seed of the destination account
            let (expected_destination_account, destination_bump) = Pubkey::find_program_address(
                &[
                    DESTINATION_SEED_PREFIX,
                    &self.order.data.dest_chain_id.to_be_bytes(),
                ],
                &ID,
            );
            let destination_account = self
                .destination_account
                .as_ref()
                .ok_or(OrderBookError::DestinationAccountRequired)?;
            require_keys_eq!(
                destination_account.key(),
                expected_destination_account,
                OrderBookError::InvalidDestinationAccount
            );
            require_eq!(
                destination_account.bump,
                destination_bump,
                OrderBookError::InvalidDestinationAccount
            );

            // Return the effective finality buffer for the destination
            let current_timestamp = Clock::get()?.unix_timestamp as u64;
            destination_account.effective_finality_buffer(current_timestamp)
        } else {
            0
        };

        let order = &self.order.data;

        // Validate the order has not been completed and that the finality buffer has passed based on the status
        let current_timestamp = Clock::get()?.unix_timestamp as u64;
        if order.status == OrderStatus::Created {
            require!(
                current_timestamp >= order.fill_deadline + finality_buffer,
                OrderBookError::FinalityPending
            )
        } else if order.status == OrderStatus::CancelRequested {
            require!(
                current_timestamp >= order.cancel_requested_at + finality_buffer,
                OrderBookError::FinalityPending
            )
        } else {
            return err!(OrderBookError::InvalidOrderStatus);
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
                &ctx.accounts.sender_token_in_ata,
                amount,
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
            amount,
        });

        Ok(())
    }
}

#[event]
pub struct RefundClaimed {
    pub order_id: [u8; 32],
    pub sender: Pubkey,
    pub amount: u64,
}
