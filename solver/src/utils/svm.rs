use anchor_client::solana_sdk::pubkey;
use anchor_client::solana_sdk::{hash::hash, instruction::AccountMeta, pubkey::Pubkey};
use m0_liquidity_sdk::types::Chain;

use crate::utils::chain_from_id;

pub const PORTAL_PROGRAM_ID: Pubkey = pubkey!("MzBrgc8yXBj4P16GTkcSyDZkEQZB9qDqf3fh9bByJce");

// Wormhole-related constants
pub const WORMHOLE_ADAPTER: Pubkey = pubkey!("mzp1q2j5Hr1QuLC3KFBCAUz5aUckT6qyuZKZ3WJnMmY");
const WORMHOLE_CORE_BRIDGE_CONFIG: Pubkey = pubkey!("2yVjuQwpsvdsrywzsJJVs9Ueh4zayyo5DYJbBNc3DDpn");
const WORMHOLE_CORE_BRIDGE_CONFIG_DEVNET: Pubkey =
    pubkey!("6bi4JGDoRwUs9TYBuvoA7dUVyikTJDrJsJU1ew6KVLiu");
const WORMHOLE_CORE_BRIDGE: Pubkey = pubkey!("worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth");
const WORMHOLE_CORE_BRIDGE_DEVNET: Pubkey = pubkey!("3u8hJUVTA4jH1wYAyUur7FFZVQ8H635K3tSHHF4ssjQ5");
const WORMHOLE_FEE_COLLECTOR: Pubkey = pubkey!("9bFNrXNb2WTx8fMHXCheaZqkLZ3YCCaiqTftHxeintHy");
const WORMHOLE_FEE_COLLECTOR_DEVNET: Pubkey =
    pubkey!("7s3a1ycs16d6SNDumaRtjcoyMaTDZPavzgsmS3uUZYWX");
const WORMHOLE_POST_MESSAGE_SHIM: Pubkey = pubkey!("EtZMZM22ViKMo4r5y4Anovs3wKQ2owUmDpjygnMMcdEX");
const CLOCK_ID: Pubkey = pubkey!("SysvarC1ock11111111111111111111111111111111");

/// Find a program derived address (PDA) for the given seeds and program ID.
pub fn find_pda(seeds: &[&[u8]], program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program_id).0
}

/// Derive the Wormhole-related accounts needed for cross-chain messaging.
pub fn derive_wormhole_accounts(destination_chain_id: u32) -> Vec<AccountMeta> {
    let devnet = chain_from_id(destination_chain_id) == Chain::SolanaDevnet;
    let emitter = find_pda(&[b"emitter"], &WORMHOLE_ADAPTER);
    let (bridge_config, bridge, fee_collector) = if devnet {
        (
            WORMHOLE_CORE_BRIDGE_CONFIG_DEVNET,
            WORMHOLE_CORE_BRIDGE_DEVNET,
            WORMHOLE_FEE_COLLECTOR_DEVNET,
        )
    } else {
        (
            WORMHOLE_CORE_BRIDGE_CONFIG,
            WORMHOLE_CORE_BRIDGE,
            WORMHOLE_FEE_COLLECTOR,
        )
    };

    vec![
        AccountMeta::new_readonly(find_pda(&[b"global"], &WORMHOLE_ADAPTER), false),
        AccountMeta::new(bridge_config, false),
        AccountMeta::new(
            find_pda(&[&emitter.to_bytes()], &WORMHOLE_POST_MESSAGE_SHIM),
            false,
        ),
        AccountMeta::new_readonly(emitter, false),
        AccountMeta::new(
            find_pda(&[b"Sequence", &emitter.to_bytes()], &bridge),
            false,
        ),
        AccountMeta::new(fee_collector, false),
        AccountMeta::new_readonly(CLOCK_ID, false),
        AccountMeta::new_readonly(bridge, false),
        AccountMeta::new_readonly(
            find_pda(&[b"__event_authority"], &WORMHOLE_POST_MESSAGE_SHIM),
            false,
        ),
        AccountMeta::new_readonly(WORMHOLE_POST_MESSAGE_SHIM, false),
    ]
}

/// Compute Anchor instruction discriminator (first 8 bytes of sha256("global:<instruction_name>"))
pub fn anchor_discriminator(instruction_name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", instruction_name);
    let hash = hash(preimage.as_bytes());
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&hash.as_ref()[..8]);
    discriminator
}
