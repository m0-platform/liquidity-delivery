use anchor_lang::prelude::*;
use crate::{
    constants::ANCHOR_DISCRIMINATOR_SIZE,
    state::{OrderBookGlobal, GLOBAL_SEED}
};
use messenger::{AUTHORITY_SEED, ID as MESSENGER_ID};

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

    /// CHECK: We derive the key from the seeds. No data is read.
    #[account(
        seeds = [AUTHORITY_SEED],
        seeds::program = MESSENGER_ID,
        bump,
    )]
    pub messenger_authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>
}

impl Initialize<'_> {
    pub fn handler(ctx: Context<Self>, chain_id: u32) -> Result<()> {
        ctx.accounts.global_account.set_inner(OrderBookGlobal {
            admin: ctx.accounts.admin.key(),
            chain_id,
            messenger_authority: ctx.accounts.messenger_authority.key(),
            bump: ctx.bumps.global_account
        });

        Ok(())
    }
}