use anchor_lang::prelude::*;

use crate::{
    constants::ANCHOR_DISCRIMINATOR_SIZE,
    error::OrderBookError,
    state::{Destination, OrderBookGlobal, DESTINATION_SEED_PREFIX, GLOBAL_SEED},
};

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

        emit_cpi!(DestinationAdded {
            dest_chain_id,
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
        

#[event]
pub struct DestinationSupportUpdated {
    pub dest_chain_id: u32,
    pub is_supported: bool,
}

// #[event_cpi]
// #[derive(Accounts)]
// #[instruction(dest_chain_id: u32)]
// pub struct ConfigureDestination<'info> {
//     #[account(mut)]
//     pub admin: Signer<'info>,

//     #[account(
//         seeds = [GLOBAL_SEED],
//         bump = global_account.bump,
//         has_one = admin @ OrderBookError::NotAuthorized,
//     )]
//     pub global_account: Account<'info, OrderBookGlobal>,

//     #[account(
//         init_if_needed,
//         payer = admin,
//         space = ANCHOR_DISCRIMINATOR_SIZE + Destination::INIT_SPACE,
//         seeds = [DESTINATION_SEED_PREFIX, &dest_chain_id.to_be_bytes()],
//         bump,
//     )]
//     pub destination_account: Account<'info, Destination>,

//     pub system_program: Program<'info, System>,
// }

// impl ConfigureDestination<'_> {
//     fn validate(&self, dest_chain_id: u32, is_supported: bool) -> Result<()> {
//         if dest_chain_id == self.global_account.chain_id {
//             return err!(OrderBookError::InvalidDestChainId);
//         }

//         Ok(())
//     }

//     #[access_control(ctx.accounts.validate(dest_chain_id, is_supported))]
//     pub fn handler(
//         ctx: Context<Self>,
//         dest_chain_id: u32,
//         is_supported: bool,
//     ) -> Result<()> {
        

//         // Case 1: no-op
//         if !ctx.accounts.destination_account.is_supported && !is_supported {
//             return Ok(());
//         }

//         let current_timestamp = Clock::get()?.unix_timestamp as u64;
//         let effective_finality_buffer = ctx.accounts.destination_account.effective_finality_buffer(current_timestamp);
        
//         let (new_finality_buffer, new_finality_buffer_effective_timestamp) = if !is_supported {
//             // Case 2: Removing support
//             // We always set the effective timestamp to now + the old finality buffer since it must be greater than zero
//             let new_finality_buffer = 0u64;
//             let new_finality_buffer_effective_timestamp = current_timestamp + effective_finality_buffer;
//             (new_finality_buffer, new_finality_buffer_effective_timestamp)
//         } else {
//             // Cases 3 and 4: Adding support or updating finality buffer
//             // We know finality buffer has been provided and is non-zero due to validation
//             let new_finality_buffer = finality_buffer.unwrap();

//             // Case 3 and 4: Adding support or updating finality buffer
//             // If reducing the finality buffer, set the effective timestamp to now + the old finality buffer
//             // This is to allow existing orders to still respect the old finality buffer
//             // If increasing the finality buffer, it can be set immediately
//             let new_finality_buffer_effective_timestamp = if new_finality_buffer < effective_finality_buffer {
//                 current_timestamp + effective_finality_buffer
//             } else {
//                 current_timestamp
//             };

//             (new_finality_buffer, new_finality_buffer_effective_timestamp)
//         };

//         ctx.accounts.destination_account.set_inner(Destination {
//             is_supported,
//             finality_buffer: effective_finality_buffer,
//             new_finality_buffer,
//             new_finality_buffer_effective_timestamp,
//             bump: ctx.bumps.destination_account,
//         });

//         emit_cpi!(DestinationConfigUpdated {
//             dest_chain_id,
//             is_supported,
//             new_finality_buffer,
//             new_finality_buffer_effective_timestamp,
//         });

//         Ok(())
//     }
// }

// #[event]
// pub struct DestinationConfigUpdated {
//     pub dest_chain_id: u32,
//     pub is_supported: bool,
//     pub new_finality_buffer: u64,
//     pub new_finality_buffer_effective_timestamp: u64,
// }


