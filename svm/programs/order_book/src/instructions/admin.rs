use anchor_lang::prelude::*;
use crate::{
    state::{OrderBookGlobal, GLOBAL_SEED},
    error::OrderBookError,
};

#[derive(Accounts)]
pub struct AdminInstruction<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = global_account.bump,
        has_one = admin @ OrderBookError::NotAuthorized,
    )]
    pub global_account: Account<'info, OrderBookGlobal>,
}

impl AdminInstruction<'_> {

    pub fn set_portal_authority(
        ctx: Context<Self>,
        new_portal_authority: Pubkey,
    ) -> Result<()> {
        ctx.accounts.global_account.portal_authority = new_portal_authority;
        Ok(())
    }

    pub fn set_new_admin(
        ctx: Context<Self>,
        new_admin: Pubkey,
    ) -> Result<()> {
        ctx.accounts.global_account.new_admin = Some(new_admin);
        Ok(())
    }

    pub fn clear_new_admin(
        ctx: Context<Self>,
    ) -> Result<()> {
        ctx.accounts.global_account.new_admin = None;
        Ok(())
    }

    pub fn pause(
        ctx: Context<Self>,
    ) -> Result<()> {
        ctx.accounts.global_account.paused = true;
        Ok(())
    }

    pub fn unpause(
        ctx: Context<Self>,
    ) -> Result<()> {
        ctx.accounts.global_account.paused = false;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct AcceptAdminRole<'info> {
    pub new_admin: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = global_account.bump,
        constraint = global_account.new_admin == Some(new_admin.key()) @ OrderBookError::NotAuthorized,
    )]
    pub global_account: Account<'info, OrderBookGlobal>,
}

impl AcceptAdminRole<'_> {
    pub fn handler(
        ctx: Context<Self>,
    ) -> Result<()> {
        ctx.accounts.global_account.admin = ctx.accounts.global_account.new_admin.unwrap();
        ctx.accounts.global_account.new_admin = None;
        Ok(())
    }
}