use crate::{
    constants::ANCHOR_DISCRIMINATOR_SIZE,
    state::{OrderBookGlobal, GLOBAL_SEED},
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = ANCHOR_DISCRIMINATOR_SIZE + OrderBookGlobal::INIT_SPACE,
        seeds = [GLOBAL_SEED],
        bump
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    pub system_program: Program<'info, System>,
}

impl Initialize<'_> {
    pub fn handler(ctx: Context<Self>, chain_id: u32, messenger_authority: Pubkey) -> Result<()> {
        ctx.accounts.global_account.set_inner(OrderBookGlobal {
            admin: ctx.accounts.admin.key(),
            new_admin: None,
            chain_id,
            messenger_authority,
            bump: ctx.bumps.global_account,
            reserved: [0u8; 128],
        });

        Ok(())
    }
}
