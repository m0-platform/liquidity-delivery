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
    pub chain_id: u32,
    pub messenger_authority: Pubkey,
    pub bump: u8,
}