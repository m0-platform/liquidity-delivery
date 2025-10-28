use alloy::primitives::Address;
use m0_liquidity_sdk::types::{Chain, ChainRuntime};

pub fn chain_id(chain: Chain) -> u32 {
    match chain {
        Chain::Ethereum => 1,
        Chain::Solana => 4294967295,
        Chain::Arbitrum => 42161,
        Chain::Optimism => 10,
        Chain::Base => 8453,
        Chain::Linea => 59144,
        Chain::Fogo => 4294967294,
        Chain::Sepolia => 11155111,
        Chain::ArbitrumSepolia => 421614,
    }
}

pub fn chain_from_id(chain_id: u32) -> Chain {
    match chain_id {
        1 => Chain::Ethereum,
        4294967295 => Chain::Solana,
        42161 => Chain::Arbitrum,
        10 => Chain::Optimism,
        8453 => Chain::Base,
        59144 => Chain::Linea,
        4294967294 => Chain::Fogo,
        11155111 => Chain::Sepolia,
        421614 => Chain::ArbitrumSepolia,
        _ => panic!("Unsupported chain ID: {}", chain_id),
    }
}

pub fn supported_chains() -> Vec<Chain> {
    vec![
        Chain::Ethereum,
        Chain::Solana,
        Chain::Arbitrum,
        Chain::Optimism,
        Chain::Base,
        Chain::Linea,
        Chain::Fogo,
        Chain::Sepolia,
        Chain::ArbitrumSepolia,
    ]
}

pub fn chain_runtime(chain_id: u32) -> ChainRuntime {
    if chain_id == 4294967295 || chain_id == 4294967294 {
        ChainRuntime::Svm
    } else {
        ChainRuntime::Evm
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
