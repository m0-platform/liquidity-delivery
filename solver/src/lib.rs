mod components;
pub mod config;
mod error;
mod events;
mod providers;
mod stores;
pub mod utils;

use components::{EvmEventListener, InventoryManager};
pub use config::Config;
use events::EventBus;
use providers::ProviderManager;
use slog::{info, Logger};
use std::{error::Error, future::Future, sync::Arc, time::Duration};
use tokio::{
    sync::broadcast::{self, Receiver},
    time::sleep,
};

use crate::{
    components::{ApiServer, ComponentParams, EvmWriter, OrderProcessor, QuoterClient, SvmEventListener},
    error::SolverError,
    events::{EventHandler, SolverEvent},
};

/// Initialize and run the solver application
/// Returns a shutdown sender that can be used to stop the application
pub async fn run_solver(
    config: Config,
    logger: Logger,
) -> Result<broadcast::Sender<()>, Box<dyn Error + Send + Sync>> {
    let environment = format!("{:?}", config.environment);
    let network = format!("{:?}", config.network);
    let chains_count = config.chains.len();
    info!(
        logger,
        "Starting Solver Application";
        "environment" => %environment,
        "network" => %network,
        "chains_count" => chains_count,
    );

    // Initialize event bus
    let event_bus = Arc::new(EventBus::new(1000));

    // Initialize shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Initialize global provider manager
    let provider_manager = Arc::new(ProviderManager::new(
        config.rate_limit.max_requests_per_second,
        config.rate_limit.burst_size,
    ));
    provider_manager.initialize(&config.chains).await?;

    // Initialize components
    let evm_listener = Arc::new(EvmEventListener::new(&params));
    let evm_writer = Arc::new(EvmWriter::new(&params));
    let svm_listener = Arc::new(SvmEventListener::new(&params));
    let order_processor = Arc::new(OrderProcessor::new(&params));
    let inventory_manager = Arc::new(InventoryManager::new(&params));
    let event_logger = Arc::new(components::EventLogger::new(&params));
    let order_timer = Arc::new(components::OrderTimer::new(&params));
    let quoter_client = Arc::new(QuoterClient::new(&params));
    let api_server = Arc::new(ApiServer::new(&params));

    // Initialize all components
    evm_listener.initialize().await?;
    svm_listener.initialize().await?;
    order_processor.initialize().await?;
    inventory_manager.initialize().await?;
    event_logger.initialize().await?;
    order_timer.initialize().await?;
    quoter_client.initialize().await?;
    api_server.initialize().await?;

    // Spawn handlers for all components
    register_component(&evm_listener, &event_bus, &shutdown_tx, &logger);
    register_component(&evm_writer, &event_bus, &shutdown_tx, &logger);
    register_component(&svm_listener, &event_bus, &shutdown_tx, &logger);
    register_component(&order_processor, &event_bus, &shutdown_tx, &logger);
    register_component(&inventory_manager, &event_bus, &shutdown_tx, &logger);
    register_component(&event_logger, &event_bus, &shutdown_tx, &logger);
    register_component(&order_timer, &event_bus, &shutdown_tx, &logger);
    register_component(&quoter_client, &event_bus, &shutdown_tx, &logger);
    register_component(&api_server, &event_bus, &shutdown_tx, &logger);

    // Give spawned tasks time to subscribe before publishing Start event
    sleep(Duration::from_millis(100)).await;
    info!(logger, "All components registered");

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
    _component_name: &'static str,
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
                    let _ = handler(SolverEvent::Stop).await;
                    break;
                },
                result = receiver.recv() => {
                    // Early continue on receive error
                    let event = match result {
                        Ok(event) => event,
                        Err(_e) => {
                            continue;
                        }
                    };

                    // Handle event and get new events
                    let new_events = match handler(event).await {
                        Ok(events) => events,
                        Err(_e) => {
                            continue;
                        }
                    };

                    // Publish new events
                    for new_event in new_events {
                        let _ = event_bus.publish(new_event).await;
                    }
                }
            }
        }
    });
}
