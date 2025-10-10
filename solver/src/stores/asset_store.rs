use async_trait::async_trait;
use m0_liquidity_sdk::types::{Asset, AssetAddress};
use m0_liquidity_sdk::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{Result, SolverError};
use crate::events::{EventHandler, SolverEvent};

/// Event store for tracking order status
pub struct AssetStore {
    assets: Arc<RwLock<HashMap<AssetAddress, Asset>>>,
    liquidity_api_url: String,
}

impl AssetStore {
    pub fn new(liquidity_api_url: String) -> Self {
        Self {
            assets: Arc::new(RwLock::new(HashMap::new())),
            liquidity_api_url,
        }
    }
}

#[async_trait]
impl EventHandler for AssetStore {
    fn name(&self) -> &'static str {
        "AssetStore"
    }

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
                AssetAddress {
                    chain: asset.chain.clone(),
                    address: asset.address.clone(),
                },
                asset,
            );
        }

        Ok(())
    }

    async fn handle_event(&self, _event: Arc<SolverEvent>) -> Result<Arc<Vec<SolverEvent>>> {
        Ok(Arc::new(vec![]))
    }
}
