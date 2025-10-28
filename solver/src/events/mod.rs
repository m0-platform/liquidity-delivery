use async_trait::async_trait;

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
    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>>;
}

/// Event handler trait for stores
#[async_trait]
pub trait EventProcessor: Send + Sync {
    // Initialize the component
    async fn initialize(&self) -> Result<()>;

    // Process events
    async fn handle_event(&self, event: SolverEvent) -> Result<()>;
}
