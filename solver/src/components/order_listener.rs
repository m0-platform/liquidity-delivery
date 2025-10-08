use async_trait::async_trait;
use order_book::OrderData;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

use crate::components::Component;
use crate::error::Result;
use crate::events::{EventHandler, OrderCreatedEvent};
use crate::{EventBus, OrderEvent, OrderStore};

/// Component that listens to new orders created
pub struct OrderListener {
    order_store: Arc<RwLock<OrderStore>>,
}

impl OrderListener {
    pub fn new() -> Self {
        Self {
            order_store: Arc::new(RwLock::new(OrderStore::new())),
        }
    }
}

#[async_trait]
impl Component for OrderListener {
    async fn initialize(&self) -> Result<()> {
        tracing::info!("Initializing");
        Ok(())
    }

    async fn start(&self, event_bus: Arc<EventBus>, mut shutdown_rx: Receiver<()>) -> Result<()> {
        tracing::info!("Starting");

        let order_store_clone = self.order_store.clone();
        let event_bus_clone = event_bus.clone();

        // Subscribe to events and handle them
        let mut receiver = event_bus.subscribe();
        let mut shutdown_rx_event = shutdown_rx.resubscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx_event.recv() => {
                        tracing::info!("Event handler received shutdown signal");
                        break;
                    }
                    result = receiver.recv() => {
                        match result {
                            Ok(event) => {
                                let store = order_store_clone.read().await;
                                if let Err(e) = store.handle_event(event).await {
                                    tracing::error!("Failed to handle event: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Error receiving event: {}", e);
                            }
                        }
                    }
                }
            }
        });

        // Task to get orders from chain
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Order polling received shutdown signal");
                        break;
                    }
                    _ = interval.tick() => {
                        // TODO: Listen for orders
                        let order = OrderData {
                            version: 0,
                            origin_chain_id: 0,
                            sender: [0u8; 32],
                            nonce: 0,
                            dest_chain_id: 0,
                            fill_deadline: 0,
                            token_out: [0u8; 32],
                            recipient: [0u8; 32],
                            amount_out: 0,
                            solver: [0u8; 32],
                        };

                        let event = OrderCreatedEvent::new(order);

                        tracing::info!("Creating order {}", event.order_id,);

                        if let Err(e) = event_bus_clone
                            .publish(Arc::new(OrderEvent::Created(event)))
                            .await
                        {
                            tracing::error!("Failed to publish event: {}", e);
                        }
                    }
                }
            }
        });

        Ok(())
    }
}
