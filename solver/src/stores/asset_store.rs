use anchor_client::solana_sdk::bs58;
use async_trait::async_trait;
use m0_liquidity_sdk::types::{Asset, Chain};
use m0_liquidity_sdk::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{Result, SolverError};
use crate::events::{EventProcessor, SolverEvent};
use crate::utils;

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
            let bytes = hex::decode(address.trim_start_matches("0x"))
                .expect("Invalid hex in asset address");

            let len = bytes.len().min(32);
            let mut addr = [0u8; 32];
            addr[32 - len..].copy_from_slice(&bytes[..len]);
            addr
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
