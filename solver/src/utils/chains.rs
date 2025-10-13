use m0_liquidity_sdk::types::Chain;

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
