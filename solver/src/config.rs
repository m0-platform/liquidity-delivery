use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

impl Environment {
    pub fn from_str(s: &str) -> Result<Self, ConfigError> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Environment::Development),
            "production" | "prod" => Ok(Environment::Production),
            _ => Err(ConfigError::InvalidEnvironment(s.to_string())),
        }
    }

    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Network {
    Local,
    Devnet,
    Mainnet,
}

impl Network {
    pub fn from_str(s: &str) -> Result<Self, ConfigError> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Network::Local),
            "devnet" => Ok(Network::Devnet),
            "mainnet" => Ok(Network::Mainnet),
            _ => Err(ConfigError::InvalidNetwork(s.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: Environment,
    pub network: Network,
    pub chains: Vec<ChainConfig>,
    pub liquidity_api_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let environment = env::var("ENV")
            .map(|s| Environment::from_str(&s))
            .unwrap()?;

        let network = env::var("NETWORK")
            .map(|s| Network::from_str(&s))
            .unwrap()?;

        let liquidity_api_url =
            env::var("LIQUIDITY_API_URL").expect("LIQUIDITY_API_URL must be set");

        // Load chain configurations from environment variables
        // Format: CHAIN_<N>_ID, CHAIN_<N>_RPC_URL, CHAIN_<N>_WS_URL, CHAIN_<N>_ORDER_BOOK_ADDRESS
        let mut chains = Vec::new();
        let mut i = 0;
        loop {
            let chain_id_key = format!("CHAIN_{}_ID", i);
            match env::var(&chain_id_key) {
                Ok(chain_id_str) => {
                    let chain_id = chain_id_str.parse::<u32>().map_err(|_| {
                        ConfigError::InvalidChainConfig(format!(
                            "Invalid chain ID: {}",
                            chain_id_str
                        ))
                    })?;

                    let rpc_url = env::var(&format!("CHAIN_{}_RPC_URL", i)).map_err(|_| {
                        ConfigError::InvalidChainConfig(format!("Missing RPC URL for chain {}", i))
                    })?;

                    let ws_url = env::var(&format!("CHAIN_{}_WS_URL", i)).ok();

                    let order_book_address = env::var(&format!("CHAIN_{}_ORDER_BOOK_ADDRESS", i))
                        .map_err(|_| {
                        ConfigError::InvalidChainConfig(format!(
                            "Missing OrderBook address for chain {}",
                            i
                        ))
                    })?;

                    chains.push(ChainConfig::new(
                        chain_id,
                        rpc_url,
                        ws_url,
                        order_book_address,
                    ));
                    i += 1;
                }
                Err(_) => break,
            }
        }

        if chains.is_empty() {
            return Err(ConfigError::InvalidChainConfig(
                "No chains configured".to_string(),
            ));
        }

        Ok(Config {
            environment,
            network,
            chains,
            liquidity_api_url,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u32,
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub order_book_address: String,
}

impl ChainConfig {
    pub fn new(
        chain_id: u32,
        rpc_url: String,
        ws_url: Option<String>,
        order_book_address: String,
    ) -> Self {
        Self {
            chain_id,
            rpc_url,
            ws_url,
            order_book_address,
        }
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid environment value: {0}. Expected 'development' or 'production'")]
    InvalidEnvironment(String),

    #[error("Invalid network value: {0}. Expected 'local', 'devnet', or 'mainnet'")]
    InvalidNetwork(String),

    #[error("Invalid chain configuration: {0}")]
    InvalidChainConfig(String),
}
