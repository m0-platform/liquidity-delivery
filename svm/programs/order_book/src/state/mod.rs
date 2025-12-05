use anchor_lang::prelude::*;

pub mod orders;
pub use orders::*;

#[constant]
pub const NONCE_SEED_PREFIX: &[u8] = b"nonce";

#[account]
#[derive(InitSpace)]
pub struct Nonce {
    pub bump: u8,
    pub value: u64,
}

#[constant]
pub const GLOBAL_SEED: &[u8] = b"global";

#[account]
#[derive(InitSpace)]
pub struct OrderBookGlobal {
    pub admin: Pubkey,
    pub new_admin: Option<Pubkey>,
    pub chain_id: u32,
    pub messenger_authority: Pubkey,
    pub bump: u8,
    pub reserved: [u8; 128], // reserved space for future upgrades
}

#[constant]
pub const DESTINATION_SEED_PREFIX: &[u8] = b"destination";

#[account]
#[derive(Debug, InitSpace)]
pub struct Destination {
    pub is_supported: bool,
    pub finality_buffer: u64,
    pub new_finality_buffer: u64,
    pub new_finality_buffer_effective_timestamp: u64,
    pub bump: u8,
}

impl Destination {
    pub fn effective_finality_buffer(&self, timestamp: u64) -> u64 {
        if timestamp >= self.new_finality_buffer_effective_timestamp
        {
            self.new_finality_buffer
        } else {
            self.finality_buffer
        }
    }
}
