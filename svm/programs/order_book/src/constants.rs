use anchor_lang::prelude::*;

#[constant]
pub const VERSION: u16 = 1;

pub const CHAIN_ID: u32 = 1; // TODO: move this into a global state so it can be set per deployment

pub const ANCHOR_DISCRIMINATOR_SIZE: usize = 8;
