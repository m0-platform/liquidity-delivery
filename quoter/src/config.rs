use serde::{Deserialize, Serialize};
use std::fs;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct QuoterConfig {
    pub chains: Vec<ChainConfig>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Asset {
    pub ticker: String,
    pub name: String,
    pub icon: String,
    pub address: String,
    pub chain_ids: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u32,
    pub enabled: bool,
    pub rpc_url: String,
    pub ws_url: String,
    pub order_book_address: String,
    /// Block number to start fetching historical events from (defaults to 0)
    #[serde(default)]
    pub starting_block: u64,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    FileRead(String),
    #[error("Failed to parse YAML: {0}")]
    ParseError(String),
    #[error("No enabled chains configured")]
    NoChains,
}

impl QuoterConfig {
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let contents =
            fs::read_to_string(path).map_err(|e| ConfigError::FileRead(e.to_string()))?;

        let config: QuoterConfig =
            serde_yaml::from_str(&contents).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        let enabled_chains: Vec<_> = config.chains.iter().filter(|c| c.enabled).collect();
        if enabled_chains.is_empty() {
            return Err(ConfigError::NoChains);
        }

        Ok(config)
    }

    pub fn enabled_chains(&self) -> Vec<ChainConfig> {
        self.chains.iter().filter(|c| c.enabled).cloned().collect()
    }
}
