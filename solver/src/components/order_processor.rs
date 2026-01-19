use async_trait::async_trait;
use slog::Logger;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::events::{EventHandler, EventProcessor, OrderRejectEvent, SolverEvent};
use crate::stores::{AssetStore, OrderStore};

pub struct OrderProcessor {
    order_store: Arc<RwLock<OrderStore>>,
    asset_store: Arc<RwLock<AssetStore>>,
    logger: Logger,
}

impl OrderProcessor {
    pub fn new(liquidity_api_url: String, logger: Logger) -> Self {
        Self {
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            asset_store: Arc::new(RwLock::new(AssetStore::new(liquidity_api_url))),
            logger,
        }
    }

    async fn get_supported_asset(
        &self,
        token_address: [u8; 32],
        chain_id: u32,
    ) -> std::result::Result<Asset, String> {
        let asset = self
            .asset_store
            .get_asset(token_address, chain_id)
            .await
            .ok_or_else(|| "Asset not supported".to_string())?;

        if !self.supported_assets.is_asset_supported(&asset) {
            return Err(format!("Asset {} not supported", asset.address));
        }

        Ok(asset)
    }

    async fn get_supported_assets(
        &self,
        input_token: [u8; 32],
        input_chain_id: u32,
        output_token: [u8; 32],
        output_id: u32,
    ) -> std::result::Result<(Asset, Asset), String> {
        let input_asset = self
            .get_supported_asset(input_token, input_chain_id)
            .await
            .map_err(|e| format!("input_token: {}", e))?;

        let output_asset = self
            .get_supported_asset(output_token, output_id)
            .await
            .map_err(|e| format!("output_token: {}", e))?;

        if !input_asset.m0_extension && !output_asset.m0_extension {
            return Err("At least one asset must be an M0 extension".to_string());
        }

        if input_asset == output_asset {
            return Err("Must be different assets or different chains".to_string());
        }

        Ok((input_asset, output_asset))
    }

    fn get_reprocess_time(&self) -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            + (self.clip_process_delay as u128 * 1000)
    }
}

#[async_trait]
impl EventHandler for OrderProcessor {
    fn name(&self) -> &'static str {
        "OrderProcessor"
    }

    async fn initialize(&self) -> Result<()> {
        self.order_store.write().await.initialize().await?;
        self.asset_store.write().await.initialize().await?;
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let store = self.order_store.read().await;
        let _ = store.handle_event(event.clone()).await;

        match event {
            SolverEvent::OrderCreated(e) => {
                let asset_store = self.asset_store.read().await;

                let source_asset = (*asset_store)
                    .get_asset(e.token_in, e.order.origin_chain_id)
                    .await?;
                let destination_asset = (*asset_store)
                    .get_asset(e.order.token_out, e.order.dest_chain_id)
                    .await?;

                if source_asset.is_none() || destination_asset.is_none() {
                    return Ok(vec![SolverEvent::OrderRejected(OrderRejectEvent::new(
                        e.order_id,
                        "Asset not supported".to_string(),
                    ))]);
                }

                // Handle order
            }
            _ => {}
        }

        Ok(vec![])
    }
}
