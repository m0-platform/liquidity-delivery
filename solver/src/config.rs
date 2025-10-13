use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;
use tracing_subscriber::filter::LevelFilter;

use crate::utils::{chain_id, supported_chains};

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
            "local" | "localnet" => Ok(Network::Local),
            "devnet" => Ok(Network::Devnet),
            "mainnet" => Ok(Network::Mainnet),
            _ => Err(ConfigError::InvalidNetwork(s.to_string())),
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            Network::Local => "localnet",
            Network::Devnet => "devnet",
            Network::Mainnet => "mainnet",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: Environment,
    pub network: Network,
    pub chains: Vec<ChainConfig>,
    pub liquidity_api_url: String,
    pub log_level: LevelFilter,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            environment: Environment::Development,
            network: Network::Local,
            chains: Vec::new(),
            liquidity_api_url: String::from("https://api-mainnet-b325.up.railway.app"),
            log_level: LevelFilter::INFO,
        }
    }
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

        let log_level = env::var("LOG_LEVEL")
            .ok()
            .and_then(|s| s.parse::<LevelFilter>().ok())
            .unwrap_or(LevelFilter::INFO);

        // Load chain configurations from environment variables
        let mut chains = Vec::new();
        for chain in supported_chains() {
            let chain_id = chain_id(chain);
            let enabled_key = format!("CHAIN_{}_ENABLED", chain_id);

            if env::var(enabled_key).unwrap_or(String::new()) != "true" {
                continue;
            }

            let rpc_url = env::var(&format!("CHAIN_{}_RPC_URL", chain_id)).map_err(|_| {
                ConfigError::InvalidChainConfig(format!("Missing RPC URL for chain {}", chain))
            })?;
            let ws_url = env::var(&format!("CHAIN_{}_WS_URL", chain_id)).map_err(|_| {
                ConfigError::InvalidChainConfig(format!("Missing WSS URL for chain {}", chain))
            })?;

            let order_book_address = env::var(&format!("CHAIN_{}_ORDER_BOOK_ADDRESS", chain_id))
                .map_err(|_| {
                    ConfigError::InvalidChainConfig(format!(
                        "Missing OrderBook address for chain {}",
                        chain
                    ))
                })?;

            chains.push(ChainConfig {
                chain_id,
                rpc_url,
                ws_url,
                order_book_address,
            });
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
            log_level,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u32,
    pub rpc_url: String,
    pub ws_url: String,
    pub order_book_address: String,
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
