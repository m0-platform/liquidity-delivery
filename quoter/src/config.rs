use serde::Deserialize;
use std::fs;
use thiserror::Error;

/// Chain type for determining transaction format
#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChainType {
    #[default]
    Evm,
    Svm,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuoterConfig {
    pub chains: Vec<ChainConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u32,
    pub enabled: bool,
    pub rpc_url: String,
    pub order_book_address: String,
    #[serde(default)]
    pub chain_type: ChainType,
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
