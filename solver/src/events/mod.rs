use async_trait::async_trait;
use std::sync::Arc;

use crate::error::Result;

pub mod event_bus;
pub mod events;
pub mod evm;
pub mod svm;

pub use event_bus::*;
pub use events::*;
pub use evm::*;
pub use svm::*;

/// Event handler trait for components
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an incoming event
    async fn handle_event(&self, event: Arc<OrderEvent>) -> Result<()>;
}
