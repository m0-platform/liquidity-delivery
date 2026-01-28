use std::str::FromStr;

use alloy::primitives::Address;
use anchor_client::solana_sdk::pubkey::Pubkey;
use m0_liquidity_sdk::types::{Chain, ChainRuntime};

// non-evm chains don't have standard chain ids
const SOLANA_CHAIN_ID: u32 = 1399811149;
const SOLANA_CHAIN_ID_DEVNET: u32 = 1399811150;

pub fn chain_id(chain: Chain) -> u32 {
    match chain {
        Chain::Ethereum => 1,
        Chain::Solana => SOLANA_CHAIN_ID,
        Chain::Arbitrum => 42161,
        Chain::Optimism => 10,
        Chain::Base => 8453,
        Chain::Linea => 59144,
        Chain::SolanaDevnet => SOLANA_CHAIN_ID_DEVNET,
        Chain::Sepolia => 11155111,
        Chain::ArbitrumSepolia => 421614,
        Chain::HyperEvm => 999,
        Chain::BinanceSmartChain => 56,
        Chain::Mantra => 5888,
        Chain::Plasma => 9745,
    }
}

pub fn chain_from_id(chain_id: u32) -> Chain {
    match chain_id {
        1 => Chain::Ethereum,
        SOLANA_CHAIN_ID => Chain::Solana,
        42161 => Chain::Arbitrum,
        10 => Chain::Optimism,
        8453 => Chain::Base,
        59144 => Chain::Linea,
        11155111 => Chain::Sepolia,
        421614 => Chain::ArbitrumSepolia,
        56 => Chain::BinanceSmartChain,
        999 => Chain::HyperEvm,
        SOLANA_CHAIN_ID_DEVNET => Chain::SolanaDevnet,
        5888 => Chain::Mantra,
        9745 => Chain::Plasma,
        _ => panic!("Unsupported chain ID: {}", chain_id),
    }
}

pub fn supported_chains() -> Vec<Chain> {
    vec![
        Chain::Ethereum,
        Chain::Solana,
        Chain::SolanaDevnet,
        Chain::Arbitrum,
        Chain::Optimism,
        Chain::Base,
        Chain::Linea,
        Chain::Sepolia,
        Chain::ArbitrumSepolia,
    ]
}

pub fn chain_runtime(chain_id: u32) -> ChainRuntime {
    if chain_id == SOLANA_CHAIN_ID || chain_id == SOLANA_CHAIN_ID_DEVNET {
        ChainRuntime::Svm
    } else {
        ChainRuntime::Evm
    }
}

pub fn decode_address(address: String, chain_id: u32) -> Option<[u8; 32]> {
    if chain_runtime(chain_id) == ChainRuntime::Svm {
        return Some(Pubkey::from_str(&address).ok()?.to_bytes());
    } else {
        let evm_address = Address::from_str(&address).ok()?;
        return Some(decode_evm_address(evm_address));
    }
}

pub fn decode_evm_address(address: Address) -> [u8; 32] {
    let mut out_bytes = [0u8; 32];
    out_bytes[12..32].copy_from_slice(address.as_slice());
    out_bytes
}

pub fn encode_evm_address(bytes: &[u8; 32]) -> Address {
    Address::from_slice(&bytes[12..32])
}

pub fn format_address(bytes: &[u8; 32]) -> String {
    let (padding, evm_bytes) = bytes.split_at(12);

    if padding.iter().all(|&b| b == 0) {
        Address::from_slice(evm_bytes).to_string()
    } else {
        Pubkey::new_from_array(*bytes).to_string()
    }
}

pub fn decode_order_id(order_id: &String) -> [u8; 32] {
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&hex::decode(order_id).unwrap());
    arr
}
