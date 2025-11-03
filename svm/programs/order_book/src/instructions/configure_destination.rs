use anchor_lang::prelude::*;

use crate::{
    constants::ANCHOR_DISCRIMINATOR_SIZE,
    error::OrderBookError,
    state::{
        DESTINATION_SEED_PREFIX, Destination,
        GLOBAL_SEED, OrderBookGlobal,
    },
};

#[derive(Accounts)]
#[instruction(dest_chain_id: u32)]
pub struct ConfigureDestination<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_account.bump,
        has_one = admin @ OrderBookError::NotAuthorized,
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(
        init_if_needed,
        payer = admin,
        space = ANCHOR_DISCRIMINATOR_SIZE + Destination::INIT_SPACE,
        seeds = [DESTINATION_SEED_PREFIX, &dest_chain_id.to_be_bytes()],
        bump,
    )]
    pub destination_account: Account<'info, Destination>,

    pub system_program: Program<'info, System>,
}


impl ConfigureDestination<'_> {
    fn validate(&self, is_supported: bool, finality_buffer: Option<u64>) -> Result<()> {
        if is_supported {
            if finality_buffer.is_none() || finality_buffer.unwrap() == 0 {
                return err!(OrderBookError::InvalidFinalityBuffer);
            }
        }
        Ok(())
    }

    #[access_control(ctx.accounts.validate(is_supported, finality_buffer))]
    pub fn handler(ctx: Context<Self>, _dest_chain_id: u32, is_supported: bool, finality_buffer: Option<u64>) -> Result<()> {
        ctx.accounts.destination_account.set_inner(Destination {
            is_supported,
            finality_buffer: finality_buffer.unwrap_or(0),
            bump: ctx.bumps.destination_account,
        });

        Ok(())
    }
}
