use async_trait::async_trait;
use std::sync::Arc;

use crate::error::Result;
use crate::events::EventBus;

pub mod inventory_manager;
pub mod order_listener;

pub use inventory_manager::InventoryManager;
pub use order_listener::OrderListener;

/// Base trait for all components in the system
#[async_trait]
pub trait Component: Send + Sync {
    fn name(&self) -> &str;

    async fn initialize(&self) -> Result<()>;

    async fn start(&self, event_bus: Arc<EventBus>) -> Result<()>;

    async fn stop(&self) -> Result<()>;
}
