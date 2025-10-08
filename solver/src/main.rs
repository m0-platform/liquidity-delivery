use solver::{
    components::{Component, InventoryManager, OrderListener},
    EventBus,
};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "solver=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Solver Application");

    // Initialize event bus
    let event_bus = Arc::new(EventBus::new(1000));

    // Initialize components
    let order_listener = Arc::new(OrderListener::new());
    let inventory_manager = Arc::new(InventoryManager::new());

    // Initialize all components
    order_listener.initialize().await?;
    inventory_manager.initialize().await?;

    // Start all components
    order_listener.start(event_bus.clone()).await?;
    inventory_manager.start(event_bus.clone()).await?;

    tracing::info!("All components started");

    // Run for a limited time for demonstration
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Stop all components
    tracing::info!("Stopping application");
    order_listener.stop().await?;
    inventory_manager.stop().await?;

    // Wait a bit for final processing
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    Ok(())
}
