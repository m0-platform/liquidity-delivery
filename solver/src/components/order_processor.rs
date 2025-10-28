use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::events::{EventHandler, EventProcessor, OrderRejectEvent, SolverEvent};
use crate::stores::{AssetStore, OrderStore};

pub struct OrderProcessor {
    order_store: Arc<RwLock<OrderStore>>,
    asset_store: Arc<RwLock<AssetStore>>,
}

impl OrderProcessor {
    pub fn new(liquidity_api_url: String) -> Self {
        Self {
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            asset_store: Arc::new(RwLock::new(AssetStore::new(liquidity_api_url))),
        }
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
