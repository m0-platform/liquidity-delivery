use alloy::signers::local::PrivateKeySigner;
use anchor_client::solana_sdk::{signature::Keypair, signer::Signer};
use m0_liquidity_sdk::types::Chain;
use serde::{Deserialize, Serialize};
use spl_token::solana_program::pubkey::Pubkey;
use std::{env, sync::Arc};
use thiserror::Error;

use crate::utils::{chain_from_id, chain_id, supported_chains};

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
    pub auto_rebalance: bool,
    pub max_order_clip_size: u64,
    pub max_clip_reprocess_delay_sec: u64,
    pub supported_assets: SupportedAssets,
    pub quoter_grpc_url: String,
    pub connect_to_quote_stream: bool,
    pub http_port: Option<u16>,
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
    auto_rebalance: Option<bool>,
    max_order_clip_size: Option<u64>,
    max_clip_reprocess_delay_sec: Option<u64>,
    supported_assets: Option<SupportedAssets>,
    quoter_grpc_url: String,
    connect_to_quote_stream: bool,
    http_port: Option<u16>,
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
    /// Maximum sustained requests per second across all chains
    pub max_requests_per_second: u32,
    /// Maximum burst capacity (tokens that can accumulate)
    pub burst_size: u32,
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
            max_order_clip_size: 250_000,
            max_clip_reprocess_delay_sec: 60,
            supported_assets: SupportedAssets::default(),
            auto_rebalance: true,
            quoter_grpc_url: String::from("http://127.0.0.1:50051"),
            connect_to_quote_stream: true,
            http_port: None,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        RateLimitConfig {
            max_requests_per_second: 100,
            burst_size: 50,
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

        // Load rate limit configuration
        let max_requests_per_second = env::var("RATE_LIMIT_MAX_RPS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(5);

        let burst_size = env::var("RATE_LIMIT_BURST_SIZE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(10);

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
                chain: chain_from_id(chain_id),
            });
        }

        if chains.is_empty() && environment != Environment::Local {
            return Err(ConfigError::InvalidConfig(
                "No enabled chains configured".to_string(),
            ));
        }

        Ok(Config {
            environment,
            network,
            chains,
            liquidity_api_url: config_file.liquidity_api_url,
            quoter_grpc_url: config_file.quoter_grpc_url,
            signers: Signers::new(evm_private_key, svm_private_key),
            connect_to_quote_stream: config_file.connect_to_quote_stream,
            ..Default::default()
        };

        // Override defaults with provided values
        if let Some(rpc_rate_limit) = config_file.rpc_rate_limit {
            config.rpc_rate_limit = rpc_rate_limit;
        }
        if let Some(max_clip_reprocess_delay_sec) = config_file.max_clip_reprocess_delay_sec {
            config.max_clip_reprocess_delay_sec = max_clip_reprocess_delay_sec;
        }
        if let Some(solver_fee_bps) = config_file.solver_fee_bps {
            config.solver_fee_bps = solver_fee_bps;
        }
        if let Some(max_order_clip_size) = config_file.max_order_clip_size {
            config.max_order_clip_size = max_order_clip_size;
        }
        if let Some(auto_rebalance) = config_file.auto_rebalance {
            config.auto_rebalance = auto_rebalance;
        }
        if let Some(supported_assets) = config_file.supported_assets {
            config.supported_assets = supported_assets;
        }
        if let Some(http_port) = config_file.http_port {
            config.http_port = Some(http_port);
        }

        Ok(config)
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

#[derive(Clone)]
pub struct Signers {
    evm_private_key: PrivateKeySigner,
    svm_private_key: Arc<Keypair>,
}

impl Signers {
    pub fn new(evm_private_key: PrivateKeySigner, svm_private_key: Arc<Keypair>) -> Self {
        Signers {
            evm_private_key,
            svm_private_key,
        }
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        let evm_private_key = env::var("EVM_PRIVATE_KEY")
            .map_err(|_| ConfigError::InvalidChainConfig("Missing EVM_PRIVATE_KEY".to_string()))?
            .parse()
            .map_err(|_| ConfigError::InvalidChainConfig("Invalid EVM_PRIVATE_KEY".to_string()))?;

        let svm_private_key = Arc::new(Keypair::from_base58_string(
            &env::var("SVM_PRIVATE_KEY").map_err(|_| {
                ConfigError::InvalidChainConfig("Missing SVM_PRIVATE_KEY".to_string())
            })?,
        ));

        Ok(Signers {
            evm_private_key,
            svm_private_key,
        })
    }

    pub fn svm_address(&self) -> Pubkey {
        self.svm_private_key.pubkey()
    }

    pub fn evm_address(&self) -> String {
        self.evm_private_key.address().to_string()
    }
}

impl Default for Signers {
    fn default() -> Self {
        Signers {
            evm_private_key: PrivateKeySigner::random(),
            svm_private_key: Arc::new(Keypair::new()),
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
