use async_trait::async_trait;
use std::sync::Arc;

use crate::error::Result;

pub mod event_bus;
pub mod events;
pub mod evm;

pub use event_bus::*;
pub use events::*;
pub use evm::*;

/// Event handler trait for components
#[async_trait]
pub trait EventHandler: Send + Sync {
    fn name(&self) -> &'static str;

    // Initialize the component
    async fn initialize(&self) -> Result<()>;

    // Handle and respond to events
    async fn handle_event(&self, event: Arc<SolverEvent>) -> Result<Arc<Vec<SolverEvent>>>;
}
