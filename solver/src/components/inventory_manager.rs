use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::components::Component;
use crate::error::Result;
use crate::EventBus;

/// Component that listens to new orders created
pub struct InventoryManager {
    running: Arc<RwLock<bool>>,
}

impl InventoryManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(RwLock::new(false)),
        }
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

    async fn start(&self, event_bus: Arc<EventBus>) -> Result<()> {
        tracing::info!("InventoryManager: Starting");

        let mut running = self.running.write().await;
        *running = true;
        drop(running);

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        tracing::info!("InventoryManager: Stopping");
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }
}
