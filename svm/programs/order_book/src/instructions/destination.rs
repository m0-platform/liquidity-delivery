use anchor_lang::prelude::*;

use crate::{
    constants::ANCHOR_DISCRIMINATOR_SIZE,
    error::OrderBookError,
    state::{Destination, OrderBookGlobal, DESTINATION_SEED_PREFIX, GLOBAL_SEED},
};

#[event]
pub struct DestinationSupportUpdated {
    pub dest_chain_id: u32,
    pub is_supported: bool,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(dest_chain_id: u32)]
pub struct AddDestination<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_account.bump,
        has_one = admin @ OrderBookError::NotAuthorized,
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(
        init,
        payer = admin,
        space = ANCHOR_DISCRIMINATOR_SIZE + Destination::INIT_SPACE,
        seeds = [DESTINATION_SEED_PREFIX, &dest_chain_id.to_be_bytes()],
        bump,
    )]
    pub destination_account: Account<'info, Destination>,

    pub system_program: Program<'info, System>,
}

impl AddDestination<'_> {
    fn validate(&self, dest_chain_id: u32) -> Result<()> {
        if dest_chain_id == self.global_account.chain_id {
            return err!(OrderBookError::InvalidDestChainId);
        }

        Ok(())
    }

    #[access_control(ctx.accounts.validate(dest_chain_id))]
    pub fn handler(
        ctx: Context<Self>,
        dest_chain_id: u32,
    ) -> Result<()> {
        ctx.accounts.destination_account.set_inner(Destination {
            is_supported: true,
            bump: ctx.bumps.destination_account,
        });

        emit_cpi!(DestinationSupportUpdated {
            dest_chain_id,
            is_supported: true,
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(dest_chain_id: u32)]
pub struct RemoveDestination<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_account.bump,
        has_one = admin @ OrderBookError::NotAuthorized,
    )]
    pub global_account: Account<'info, OrderBookGlobal>,

    #[account(
        mut,
        close = admin,
        seeds = [DESTINATION_SEED_PREFIX, &dest_chain_id.to_be_bytes()],
        bump = destination_account.bump,
    )]
    pub destination_account: Account<'info, Destination>,

    pub system_program: Program<'info, System>,
}

impl RemoveDestination<'_> {
    pub fn handler(
        ctx: Context<Self>,
        dest_chain_id: u32,
    ) -> Result<()> {
        emit_cpi!(DestinationSupportUpdated {
            dest_chain_id,
            is_supported: false,
        });

        Ok(())
    }
}
        


