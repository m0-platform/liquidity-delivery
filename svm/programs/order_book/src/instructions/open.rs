use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use crate::{
    state::{
        Order, OrderData, OrderType, NativeOrder, OrderStatus, ORDER_SEED_PREFIX,
        Nonce, NONCE_SEED_PREFIX, OrderBookGlobal, GLOBAL_SEED
    },
    utils::transfer_tokens,
    constants::{VERSION, ANCHOR_DISCRIMINATOR_SIZE},
    error::OrderBookError,
};
use std::ops::Deref;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OrderParams {
    pub dest_chain_id: u32,
    pub token_out: [u8; 32],
    pub amount_in: u64,
    pub amount_out: u128,
    pub recipient: [u8; 32],
    pub fill_deadline: u64,
    pub solver: [u8; 32],
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(params: OrderParams)]
pub struct OpenOrder<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Note: optional extra authority to separate token permissions from submitting the order
    /// If None, the payer is used as the token authority
    pub token_authority: Option<Signer<'info>>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_account.bump
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(mint::token_program = token_in_program)]
    pub token_in_mint: InterfaceAccount<'info, Mint>,
    
    #[account(
        mut,
        token::mint = token_in_mint,
        token::token_program = token_in_program,   
    )]
    pub sender_token_in_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = payer,
        space = ANCHOR_DISCRIMINATOR_SIZE + Nonce::INIT_SPACE,
        seeds = [NONCE_SEED_PREFIX, sender_token_in_account.deref().owner.as_ref()],
        bump
    )]
    pub sender_nonce_account: Account<'info, Nonce>,

    #[account(
        init,
        payer = payer,
        space = ANCHOR_DISCRIMINATOR_SIZE + Order::<NativeOrder>::INIT_SPACE,
        seeds = [
            ORDER_SEED_PREFIX, 
            &OrderData {
                version: VERSION as u16,
                origin_chain_id: global_account.chain_id,
                sender: sender_token_in_account.deref().owner.to_bytes(),
                nonce: sender_nonce_account.value,
                dest_chain_id: params.dest_chain_id,
                fill_deadline: params.fill_deadline,
                token_out: params.token_out,
                recipient: params.recipient,
                amount_out: params.amount_out,
                solver: params.solver,
            }.compute_order_id()
        ],
        bump
    )]
    pub order: Account<'info, Order::<NativeOrder>>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = token_in_mint,
        associated_token::authority = order,
        associated_token::token_program = token_in_program,
    )]
    pub order_token_in_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_in_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    pub system_program: Program<'info, System>,
}

impl OpenOrder<'_> {
    fn validate(&self, params: &OrderParams) -> Result<()> {
        // Validate params
        require!(params.amount_in > 0, OrderBookError::InvalidAmountIn);
        require!(params.amount_out > 0, OrderBookError::InvalidAmountOut);
        require!(params.fill_deadline > Clock::get()?.unix_timestamp as u64, OrderBookError::InvalidFillDeadline);

        Ok(())
    }

    #[access_control(ctx.accounts.validate(&params))]
    pub fn handler(ctx: Context<Self>, params: OrderParams) -> Result<()> {
        let sender: Pubkey = (&ctx.accounts.sender_token_in_account).owner;

        // Populate the order data
        ctx.accounts.order.set_inner(Order {
            order_type: OrderType::Native,
            bump: ctx.bumps.order,
            data: NativeOrder {
                status: OrderStatus::Created,
                version: VERSION,
                dest_chain_id: params.dest_chain_id,
                fill_deadline: params.fill_deadline,
                nonce: ctx.accounts.sender_nonce_account.value,
                token_in: ctx.accounts.token_in_mint.key(),
                token_out: params.token_out,
                sender,
                recipient: params.recipient,
                amount_in: params.amount_in,
                amount_out: params.amount_out,
                amount_out_filled: 0,
                solver: params.solver,
            },
        });

        let order_id = OrderData {
            version: VERSION,
            origin_chain_id: ctx.accounts.global_account.chain_id,
            sender: sender.to_bytes(),
            nonce: ctx.accounts.sender_nonce_account.value,
            dest_chain_id: params.dest_chain_id,
            fill_deadline: params.fill_deadline,
            token_out: params.token_out,
            recipient: params.recipient,
            amount_out: params.amount_out,
            solver: params.solver,
        }.compute_order_id();

        // Increment the sender's nonce account
        ctx.accounts.sender_nonce_account.value += 1;

        // Transfer the amount in from the sender to the order's token account
        let auth = match &ctx.accounts.token_authority {
            Some(signer) => signer.to_account_info(),
            None => ctx.accounts.payer.to_account_info(),
        };

        transfer_tokens(
            &ctx.accounts.sender_token_in_account,
            &ctx.accounts.order_token_in_ata,
            params.amount_in,
            &ctx.accounts.token_in_mint,
            &auth,
            &ctx.accounts.token_in_program,
        )?;

        // Emit the event
        emit_cpi!(
            OrderOpened {
                order_id,
                token_in: ctx.accounts.token_in_mint.key(),
                amount_in: params.amount_in,
                dest_chain_id: params.dest_chain_id,
                token_out: params.token_out,
                amount_out: params.amount_out,
                solver: params.solver,
            }
        );

        Ok(())
    }
}

#[event]
pub struct OrderOpened {
    pub order_id: [u8; 32],
    pub token_in: Pubkey,
    pub amount_in: u64,
    pub dest_chain_id: u32,
    pub token_out: [u8; 32],
    pub amount_out: u128,
    pub solver: [u8; 32],
}
