mod components;
mod config;
mod error;
mod events;
mod stores;
mod utils;

use components::{EvmEventListener, InventoryManager};
use config::Config;
use events::EventBus;
use std::{error::Error, future::Future, sync::Arc};
use tokio::sync::broadcast::{self, Receiver};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    components::{OrderProcessor, SvmEventListener},
    error::SolverError,
    events::{EventHandler, SolverEvent},
};

/// Initialize and run the solver application
/// Returns a shutdown sender that can be used to stop the application
pub async fn run_solver() -> Result<broadcast::Sender<()>, Box<dyn Error>> {
    let _ = dotenvy::dotenv();
    let config = Config::from_env()?;

    if config.environment.is_production() {
        // JSON format for production
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().json())
            .with(config.log_level)
            .init();
    } else {
        // Human-readable format for development
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(config.log_level)
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
    register_component(&evm_listener, &event_bus, &shutdown_tx);
    register_component(&svm_listener, &event_bus, &shutdown_tx);
    register_component(&order_processor, &event_bus, &shutdown_tx);
    register_component(&inventory_manager, &event_bus, &shutdown_tx);
    register_component(&event_logger, &event_bus, &shutdown_tx);

    tracing::info!("All components registered");
    let _ = event_bus.publish(SolverEvent::Start).await;

    Ok(shutdown_tx)
}

/// Helper function to register a component with the event bus
fn register_component<T>(
    component: &Arc<T>,
    event_bus: &Arc<EventBus>,
    shutdown_tx: &broadcast::Sender<()>,
) where
    T: EventHandler + 'static,
{
    let component_clone = component.clone();
    spawn_event_handler(
        component.name(),
        event_bus.clone(),
        shutdown_tx.subscribe(),
        move |event| {
            let c = component_clone.clone();
            async move { c.handle_event(event).await }
        },
    );
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
