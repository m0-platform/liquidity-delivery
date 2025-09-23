use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        seeds = [GLOBAL_SEED],
        bump,
        payer = admin,
        space = ANCHOR_DISCRIMINATOR_SIZE + OrderBookGlobal::SIZE,
    )]
    pub global: Account<'info, OrderBookGlobal>,
}

impl Initialize<'_> {
    pub fn handler(ctx: Context<Self>, chain_id: u32, messenger: Pubkey) -> Result<()> {
        ctx.accounts.global.set_inner(OrderBookGlobal {
            admin: ctx.accounts.admin.key(),
            chain_id,
            messenger,
            bump: ctx.bumps.global
        });
        
        Ok(())
    }
}