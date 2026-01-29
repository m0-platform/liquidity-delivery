use anchor_client::solana_sdk::pubkey;
use anchor_client::solana_sdk::{hash::hash, pubkey::Pubkey};

pub const PORTAL_PROGRAM_ID: Pubkey = pubkey!("MzBrgc8yXBj4P16GTkcSyDZkEQZB9qDqf3fh9bByJce");
pub const WORMHOLE_ADAPTER: Pubkey = pubkey!("mzp1q2j5Hr1QuLC3KFBCAUz5aUckT6qyuZKZ3WJnMmY");

/// Find a program derived address (PDA) for the given seeds and program ID.
pub fn find_pda(seeds: &[&[u8]], program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program_id).0
}

/// Compute Anchor instruction discriminator (first 8 bytes of sha256("global:<instruction_name>"))
pub fn anchor_discriminator(instruction_name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", instruction_name);
    let hash = hash(preimage.as_bytes());
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&hash.as_ref()[..8]);
    discriminator
}
