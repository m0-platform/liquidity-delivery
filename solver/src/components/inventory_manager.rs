use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::components::Component;
use crate::error::Result;
use crate::EventBus;

/// Component that listens to new orders created
pub struct InventoryManager {}

impl InventoryManager {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Component for InventoryManager {
    fn name(&self) -> &str {
        "InventoryManager"
    }

    async fn initialize(&self) -> Result<()> {
        tracing::info!("InventoryManager: Initializing");
        Ok(())
    }

    async fn start(
        &self,
        _event_bus: Arc<EventBus>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        tracing::info!("InventoryManager: Starting");

        tokio::spawn(async move {
            let _ = shutdown_rx.recv().await;
            tracing::info!("InventoryManager: Received shutdown signal");
        });

        Ok(())
    }
}
