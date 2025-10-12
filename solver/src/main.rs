mod components;
mod config;
mod error;
mod events;
mod stores;

use components::{EvmEventListener, InventoryManager};
use config::Config;
use events::EventBus;
use std::{error::Error, future::Future, sync::Arc};
use tokio::sync::broadcast::{self, Receiver};
use tracing::event;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    components::{OrderProcessor, SvmEventListener},
    error::SolverError,
    events::{EventHandler, SolverEvent},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = dotenvy::dotenv();
    let config = Config::from_env()?;

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
    let evm_listener = Arc::new(EvmEventListener::new(
        event_bus.clone(),
        config.chains.clone(),
    ));
    let svm_listener = Arc::new(SvmEventListener::new(
        event_bus.clone(),
        config.chains.clone(),
        config.network,
    ));
    let order_processor = Arc::new(OrderProcessor::new(config.liquidity_api_url.clone()));
    let inventory_manager = Arc::new(InventoryManager::new());
    let event_logger = Arc::new(components::EventLogger::new());

    // Initialize all components
    evm_listener.initialize().await?;
    svm_listener.initialize().await?;
    order_processor.initialize().await?;
    inventory_manager.initialize().await?;
    event_logger.initialize().await?;

    // Spawn handlers for all components
    let evm_listener_clone = evm_listener.clone();
    spawn_event_handler(
        evm_listener.name(),
        event_bus.clone(),
        shutdown_tx.subscribe(),
        move |event| {
            let listener = evm_listener_clone.clone();
            async move { listener.handle_event(event).await }
        },
    );

    let svm_listener_clone = svm_listener.clone();
    spawn_event_handler(
        svm_listener.name(),
        event_bus.clone(),
        shutdown_tx.subscribe(),
        move |event| {
            let listener = svm_listener_clone.clone();
            async move { listener.handle_event(event).await }
        },
    );

    let order_processor_clone = order_processor.clone();
    spawn_event_handler(
        order_processor.name(),
        event_bus.clone(),
        shutdown_tx.subscribe(),
        move |event| {
            let processor = order_processor_clone.clone();
            async move { processor.handle_event(event).await }
        },
    );

    let inventory_manager_clone = inventory_manager.clone();
    spawn_event_handler(
        inventory_manager.name(),
        event_bus.clone(),
        shutdown_tx.subscribe(),
        move |event| {
            let manager = inventory_manager_clone.clone();
            async move { manager.handle_event(event).await }
        },
    );

    let event_logger_clone = event_logger.clone();
    spawn_event_handler(
        event_logger.name(),
        event_bus.clone(),
        shutdown_tx.subscribe(),
        move |event| {
            let logger = event_logger_clone.clone();
            async move { logger.handle_event(event).await }
        },
    );

    tracing::info!("All components registered");
    let _ = event_bus.publish(SolverEvent::Start).await;

    // Wait for SIGINT (Ctrl+C)
    tokio::signal::ctrl_c().await?;
    tracing::info!("Received shutdown signal");
    let _ = shutdown_tx.send(());
    let _ = event_bus.publish(SolverEvent::Stop).await;

    // Wait for components to shutdown gracefully
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    Ok(())
}

fn spawn_event_handler<F, Fut>(
    component_name: &'static str,
    event_bus: Arc<EventBus>,
    mut shutdown_rx: Receiver<()>,
    handler: F,
) where
    F: Fn(SolverEvent) -> Fut + Send + 'static,
    Fut: Future<Output = Result<Vec<SolverEvent>, SolverError>> + Send + 'static,
{
    tokio::spawn(async move {
        let mut receiver = event_bus.subscribe();
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    tracing::info!("Shutting down event handler for {}", component_name);
                    break;
                },
                result = receiver.recv() => {
                    match result {
                        Ok(event) => {
                            match handler(event).await {
                                Ok(new_events) => {
                                    for new_event in new_events.iter() {
                                        if let Err(e) = event_bus.publish(new_event.clone()).await {
                                            tracing::error!("Failed to publish event from {}: {}", component_name, e);
                                        }
                                    }
                                }
                                Err(e) => tracing::error!("{} failed to handle event: {}", component_name, e),
                            }
                        }
                        Err(e) =>  tracing::error!("Error receiving event on {}: {}", component_name, e)
                    }
                }
            }
        }
    });
}
