use async_trait::async_trait;
use std::sync::Arc;

use crate::error::Result;
use crate::events::EventBus;

pub mod order_producer;
pub mod order_processor;
pub mod order_confirmer;

pub use order_producer::OrderProducer;
pub use order_processor::OrderProcessor;
pub use order_confirmer::OrderConfirmer;

/// Base trait for all components in the system
#[async_trait]
pub trait Component: Send + Sync {
    /// Get the component name
    fn name(&self) -> &str;
    
    /// Initialize the component
    async fn initialize(&self) -> Result<()>;
    
    /// Start the component (begin processing)
    async fn start(&self, event_bus: Arc<EventBus>) -> Result<()>;
    
    /// Stop the component
    async fn stop(&self) -> Result<()>;
}
