use alloy::hex::FromHex;
use alloy::primitives::Address;
use anchor_client::solana_sdk::bs58;
use async_trait::async_trait;
use m0_liquidity_sdk::types::{Asset, Chain, ChainRuntime};
use m0_liquidity_sdk::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{Result, SolverError};
use crate::events::{EventProcessor, SolverEvent};
use crate::utils::{self, decode_evm_address, encode_evm_address};

/// Event store for tracking order status
pub struct AssetStore {
    assets: Arc<RwLock<HashMap<AssetKey, Asset>>>,
    liquidity_api_url: String,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct AssetKey {
    address: [u8; 32],
    chain_id: u32,
}

impl AssetStore {
    pub fn new(liquidity_api_url: String) -> Self {
        Self {
            assets: Arc::new(RwLock::new(HashMap::new())),
            liquidity_api_url,
        }
    }

    pub async fn get_asset(&self, address: [u8; 32], chain_id: u32) -> Result<Option<Asset>> {
        let assets = self.assets.read().await;
        Ok(assets.get(&AssetKey { address, chain_id }).cloned())
    }

    pub async fn get_assets_for_chain(&self, chain_id: u32) -> Result<Vec<Asset>> {
        let assets = self.assets.read().await;
        Ok(assets
            .iter()
            .filter(|(key, _)| key.chain_id == chain_id)
            .map(|(_, asset)| asset.clone())
            .collect())
    }

    pub fn get_native(chain: Chain) -> Asset {
        match chain {
            Chain::Solana => Asset {
                chain,
                address: String::default(),
                decimals: 9,
                icon: String::default(),
                m0_extension: false,
                name: String::from("Solana"),
                runtime: ChainRuntime::Svm,
                symbol: String::from("SOL"),
            },
            _ => Asset {
                chain,
                address: String::default(),
                decimals: 18,
                icon: String::default(),
                m0_extension: false,
                name: String::from("Ethereum"),
                runtime: ChainRuntime::Evm,
                symbol: String::from("ETH"),
            },
        }
    }

    fn parse_address(chain: Chain, address: String) -> [u8; 32] {
        if chain == Chain::Solana {
            let bytes = bs58::decode(address)
                .into_vec()
                .expect("Invalid base58 in Solana asset address");

            let len = bytes.len().min(32);
            let mut addr = [0u8; 32];
            addr[..len].copy_from_slice(&bytes[..len]);
            addr
        } else {
            let addr = Address::from_hex(address).expect("Invalid hex in asset address");
            decode_evm_address(addr)
        }
    }
}

#[async_trait]
impl EventProcessor for AssetStore {
    async fn initialize(&self) -> Result<()> {
        let client = Client::new(&self.liquidity_api_url);

        // Get all supported assets
        let response = client
            .quote_get_supported_assets()
            .await
            .map_err(|e| SolverError::Store(format!("Failed to fetch supported assets: {}", e)))?;

        let mut assets = self.assets.write().await;

        for asset in response.into_inner() {
            assets.insert(
                AssetKey {
                    chain_id: utils::chain_id(asset.chain),
                    address: Self::parse_address(asset.chain, asset.address.clone()),
                },
                asset,
            );
        }

        tracing::info!("Loaded {} assets into AssetStore", assets.len());
        Ok(())
    }

    async fn handle_event(&self, _event: SolverEvent) -> Result<()> {
        Ok(())
    }
}
