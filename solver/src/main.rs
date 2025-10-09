mod components;
mod config;
mod error;
mod events;
mod stores;

use components::{Component, EvmEventListener, InventoryManager};
use config::Config;
use events::EventBus;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if it exists
    let _ = dotenvy::dotenv();

    // Load configuration from environment variables
    let config = Config::from_env()?;

    // Initialize tracing with appropriate format based on environment
    if config.environment.is_production() {
        // JSON format for production
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        // Human-readable format for development
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    tracing::info!(
        environment = ?config.environment,
        network = ?config.network,
        chains_count = config.chains.len(),
        "Starting Solver Application"
    );

    // Initialize event bus
    let event_bus = Arc::new(EventBus::new(1000));

    // Initialize shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Initialize components
    let order_listener = Arc::new(EvmEventListener::new(config.chains.clone()));
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
