use async_trait::async_trait;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

use crate::error::Result;
use crate::events::{EventBus, OrderEvent};

pub mod evm_event_listener;
pub mod inventory_manager;

pub use evm_event_listener::EvmEventListener;
pub use inventory_manager::InventoryManager;

/// Base trait for all components in the system
#[async_trait]
pub trait Component: Send + Sync {
    fn name() -> &'static str;

    async fn initialize(&self) -> Result<()>;

    async fn start(&self, event_bus: Arc<EventBus>, shutdown_rx: Receiver<()>) -> Result<()>;

    /// Helper method to spawn a task that subscribes to events and handles shutdown
    fn spawn_event_handler<F, Fut>(
        event_bus: Arc<EventBus>,
        mut shutdown_rx: Receiver<()>,
        handler: F,
    ) where
        F: Fn(Arc<OrderEvent>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        tokio::spawn(async move {
            let mut receiver = event_bus.subscribe();
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    result = receiver.recv() => {
                        match result {
                            Ok(event) => {
                                if let Err(e) = handler(event).await {
                                    tracing::error!("{} failed to handle event: {}", Self::name(), e);
                                }
                            }
                            Err(e) =>  tracing::error!("Error receiving event on {}: {}", Self::name(), e)
                        }
                    }
                }
            }
        });
    }
}
