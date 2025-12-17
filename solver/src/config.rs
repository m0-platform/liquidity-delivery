use anchor_client::solana_sdk::signature::Keypair;
use m0_liquidity_sdk::types::{Asset, Chain};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, sync::Arc};
use thiserror::Error;

use crate::{providers::Signers, utils::chain_from_id};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
    Local,
}

impl Environment {
    pub fn from_str(s: &str) -> Result<Self, ConfigError> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Environment::Development),
            "production" | "prod" => Ok(Environment::Production),
            "local" | "localnet" => Ok(Environment::Local),
            _ => Err(ConfigError::InvalidEnvironment(s.to_string())),
        }
    }
}

impl Default for Environment {
    fn default() -> Self {
        Environment::Development
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
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

#[derive(Clone)]
pub struct Config {
    pub environment: Environment,
    pub network: Network,
    pub chains: Vec<ChainConfig>,
    pub liquidity_api_url: String,
    pub signers: Signers,
    pub rpc_rate_limit: RateLimitConfig,
    pub solver_fee_bps: u32,
    pub max_order_clip_size: u64,
    pub supported_assets: SupportedAssets,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    environment: String,
    network: String,
    chains: Vec<ChainConfigFile>,
    liquidity_api_url: String,
    evm_private_key: String,
    svm_private_key: String,
    rpc_rate_limit: Option<RateLimitConfig>,
    solver_fee_bps: Option<u32>,
    max_order_clip_size: Option<u64>,
    supported_assets: Option<SupportedAssets>,
}

#[derive(Debug, Deserialize)]
struct ChainConfigFile {
    chain_id: u32,
    enabled: bool,
    rpc_url: String,
    ws_url: String,
    order_book_address: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RateLimitConfig {
    pub max_requests_per_second: u32,
    pub burst_size: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SupportedAssets {
    pub third_party_whitelist: Vec<String>,
    pub first_party_blacklist: Vec<String>,
}

impl SupportedAssets {
    pub fn is_asset_supported(&self, asset: &Asset) -> bool {
        let address_lower = asset.address.to_lowercase();
        if asset.m0_extension {
            return !self
                .first_party_blacklist
                .iter()
                .any(|a| a.to_lowercase() == address_lower);
        } else {
            return self
                .third_party_whitelist
                .iter()
                .any(|a| a.to_lowercase() == address_lower);
        }
    }
}

impl Default for SupportedAssets {
    fn default() -> Self {
        SupportedAssets {
            third_party_whitelist: Vec::new(),
            first_party_blacklist: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            environment: Environment::Development,
            network: Network::Local,
            chains: Vec::new(),
            liquidity_api_url: String::from("https://api-mainnet-b325.up.railway.app"),
            signers: Signers::default(),
            rpc_rate_limit: RateLimitConfig::default(),
            solver_fee_bps: 0,
            max_order_clip_size: 10_000,
            supported_assets: SupportedAssets::default(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        RateLimitConfig {
            max_requests_per_second: 10,
            burst_size: 15,
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(|e| {
            ConfigError::InvalidChainConfig(format!("Failed to read config file: {}", e))
        })?;

        let config_file: ConfigFile = serde_yaml::from_str(&contents)
            .map_err(|e| ConfigError::InvalidChainConfig(format!("Failed to parse YAML: {}", e)))?;

        let environment = Environment::from_str(&config_file.environment)?;
        let network = Network::from_str(&config_file.network)?;

        // Filter enabled chains and convert to ChainConfig
        let chains: Vec<ChainConfig> = config_file
            .chains
            .into_iter()
            .filter(|c| c.enabled)
            .map(|c| ChainConfig {
                chain_id: c.chain_id,
                chain: chain_from_id(c.chain_id),
                rpc_url: c.rpc_url,
                ws_url: c.ws_url,
                order_book_address: c.order_book_address,
            })
            .collect();

        if chains.is_empty() {
            return Err(ConfigError::InvalidChainConfig(
                "No enabled chains configured".to_string(),
            ));
        }

        // Parse signers
        let evm_private_key = config_file
            .evm_private_key
            .parse()
            .map_err(|_| ConfigError::InvalidChainConfig("Invalid EVM_PRIVATE_KEY".to_string()))?;

        let svm_private_key = Arc::new(Keypair::from_base58_string(&config_file.svm_private_key));

        Ok(Config {
            environment,
            network,
            chains,
            liquidity_api_url: config_file.liquidity_api_url,
            signers: Signers::new(evm_private_key, svm_private_key),
            rpc_rate_limit: config_file.rpc_rate_limit.unwrap_or_default(),
            solver_fee_bps: config_file.solver_fee_bps.unwrap_or(0),
            max_order_clip_size: config_file.max_order_clip_size.unwrap_or(10_000),
            supported_assets: config_file.supported_assets.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u32,
    pub chain: Chain,
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
