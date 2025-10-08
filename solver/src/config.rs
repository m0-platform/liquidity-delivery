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
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let environment = env::var("ENV")
            .map(|s| Environment::from_str(&s))
            .unwrap()?;

        let network = env::var("NETWORK")
            .map(|s| Network::from_str(&s))
            .unwrap()?;

        Ok(Config {
            environment,
            network,
        })
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid environment value: {0}. Expected 'development' or 'production'")]
    InvalidEnvironment(String),

    #[error("Invalid network value: {0}. Expected 'local', 'devnet', or 'mainnet'")]
    InvalidNetwork(String),
}
