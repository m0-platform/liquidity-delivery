pub mod event_bus;
pub mod events;
pub mod evm;

use crate::error::Result;
use async_trait::async_trait;
pub use event_bus::*;
pub use events::*;
pub use evm::*;
use std::sync::Arc;

/// Event handler trait for components
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an incoming event
    async fn handle_event(&self, event: Arc<OrderEvent>) -> Result<()>;
}
