use solver::{
    components::{Component, InventoryManager, OrderListener},
    EventBus,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Solver Application");

    // Initialize event bus
    let event_bus = Arc::new(EventBus::new(1000));

    // Initialize shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Initialize components
    let order_listener = Arc::new(OrderListener::new());
    let inventory_manager = Arc::new(InventoryManager::new());

    // Initialize all components
    order_listener.initialize().await?;
    inventory_manager.initialize().await?;

    // Start all components
    order_listener
        .start(event_bus.clone(), shutdown_tx.subscribe())
        .await?;
    inventory_manager
        .start(event_bus.clone(), shutdown_tx.subscribe())
        .await?;

    tracing::info!("All components started");

    // Wait for SIGINT (Ctrl+C)
    tokio::signal::ctrl_c().await?;
    tracing::info!("Received shutdown signal");
    let _ = shutdown_tx.send(());

    // Wait for components to shutdown gracefully
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    Ok(())
}
