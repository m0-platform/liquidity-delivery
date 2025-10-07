use std::sync::Arc;
use solver::{
    components::{Component, OrderProducer, OrderProcessor, OrderConfirmer},
    events::EventBus,
    stores::{EventStore, Store},
    EventHandler,
};
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
    
    // Initialize stores
    let event_store = Arc::new(EventStore::new());
    event_store.initialize().await?;
    
    // Register store as event handler (stores update before components process)
    event_bus.register_handler(event_store.clone() as Arc<dyn EventHandler>).await;
    
    // Initialize components
    let order_producer = Arc::new(OrderProducer::new());
    let order_processor = Arc::new(OrderProcessor::new(event_store.clone()));
    let order_confirmer = Arc::new(OrderConfirmer::new(event_store.clone()));
    
    // Initialize all components
    order_producer.initialize().await?;
    order_processor.initialize().await?;
    order_confirmer.initialize().await?;
    
    // Start all components
    order_processor.start(event_bus.clone()).await?;
    order_confirmer.start(event_bus.clone()).await?;
    order_producer.start(event_bus.clone()).await?;
    
    tracing::info!("All components started");
    
    // Run for a limited time for demonstration
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    
    // Stop all components
    tracing::info!("Stopping application");
    order_producer.stop().await?;
    order_processor.stop().await?;
    order_confirmer.stop().await?;
    
    // Wait a bit for final processing
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    
    // Print statistics
    tracing::info!("Final Statistics:");
    let counts = event_store.get_state_counts().await?;
    for (state, count) in counts {
        tracing::info!("  {}: {}", state, count);
    }
    
    let all_orders = event_store.get_all_orders().await?;
    tracing::info!("Total orders processed: {}", all_orders.len());
    
    Ok(())
}
