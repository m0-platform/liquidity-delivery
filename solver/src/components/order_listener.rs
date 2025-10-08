use async_trait::async_trait;
use order_book::OrderData;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::components::Component;
use crate::error::Result;
use crate::events::{EventHandler, OrderCreatedEvent};
use crate::{EventBus, OrderEvent, OrderStore};

/// Component that listens to new orders created
pub struct OrderListener {
    running: Arc<RwLock<bool>>,
    order_store: Arc<RwLock<OrderStore>>,
}

impl OrderListener {
    pub fn new() -> Self {
        Self {
            running: Arc::new(RwLock::new(false)),
            order_store: Arc::new(RwLock::new(OrderStore::new())),
        }
    }
}

#[async_trait]
impl Component for OrderListener {
    fn name(&self) -> &str {
        "OrderListener"
    }

    async fn initialize(&self) -> Result<()> {
        tracing::info!("OrderListener: Initializing");
        Ok(())
    }

    async fn start(&self, event_bus: Arc<EventBus>) -> Result<()> {
        tracing::info!("OrderListener: Starting");

        let mut running = self.running.write().await;
        *running = true;
        drop(running);

        let running_clone = self.running.clone();
        let order_store_clone = self.order_store.clone();
        let event_bus_clone = event_bus.clone();

        // Subscribe to events and handle them
        let mut receiver = event_bus.subscribe();
        tokio::spawn(async move {
            loop {
                let is_running = *running_clone.read().await;
                if !is_running {
                    break;
                }

                match receiver.recv().await {
                    Ok(event) => {
                        let store = order_store_clone.read().await;
                        if let Err(e) = store.handle_event(event).await {
                            tracing::error!("OrderListener: Failed to handle event: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("OrderListener: Error receiving event: {}", e);
                    }
                }
            }
        });

        let running_clone = self.running.clone();

        // Task to get orders from chain
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));

            loop {
                let is_running = *running_clone.read().await;
                if !is_running {
                    break;
                }

                interval.tick().await;

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

                tracing::info!("OrderListener: Creating order {}", event.order_id,);

                if let Err(e) = event_bus_clone
                    .publish(Arc::new(OrderEvent::Created(event)))
                    .await
                {
                    tracing::error!("OrderListener: Failed to publish event: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        tracing::info!("OrderProducer: Stopping");
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }
}
