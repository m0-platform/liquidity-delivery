use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::components::Component;
use crate::error::Result;
use crate::events::{EventBus, EventHandler, OrderEvent, OrderConfirmedEvent};
use crate::stores::EventStore;

/// Component that confirms orders have been processed
pub struct OrderConfirmer {
    event_store: Arc<EventStore>,
    running: Arc<RwLock<bool>>,
}

impl OrderConfirmer {
    pub fn new(event_store: Arc<EventStore>) -> Self {
        Self {
            event_store,
            running: Arc::new(RwLock::new(false)),
        }
    }
}

#[async_trait]
impl Component for OrderConfirmer {
    fn name(&self) -> &str {
        "OrderConfirmer"
    }
    
    async fn initialize(&self) -> Result<()> {
        tracing::info!("OrderConfirmer: Initializing");
        Ok(())
    }
    
    async fn start(&self, event_bus: Arc<EventBus>) -> Result<()> {
        tracing::info!("OrderConfirmer: Starting");
        
        let mut running = self.running.write().await;
        *running = true;
        drop(running);
        
        // Register as event handler
        let handler = Arc::new(OrderConfirmerHandler {
            event_store: self.event_store.clone(),
            event_bus: event_bus.clone(),
        });
        
        event_bus.register_handler(handler).await;
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        tracing::info!("OrderConfirmer: Stopping");
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }
}

/// Handler for confirming processed orders
struct OrderConfirmerHandler {
    event_store: Arc<EventStore>,
    event_bus: Arc<EventBus>,
}

#[async_trait]
impl EventHandler for OrderConfirmerHandler {
    async fn handle_event(&self, event: Arc<OrderEvent>) -> Result<()> {
        match event.as_ref() {
            OrderEvent::Processed(e) => {
                let order_id = e.order_id;
                
                // Verify the order exists and is in processed state
                if let Ok(Some(order)) = self.event_store.get_order(&order_id).await {
                    tracing::info!(
                        "OrderConfirmer: Confirming order {} (amount: ${:.2})",
                        order_id,
                        order.amount
                    );
                    
                    // Simulate confirmation delay (e.g., blockchain confirmation)
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                    
                    // Publish confirmation event
                    let confirmed_event = OrderConfirmedEvent::new(order_id);
                    self.event_bus.publish(Arc::new(OrderEvent::Confirmed(confirmed_event))).await?;
                    
                    tracing::info!("OrderConfirmer: Order {} confirmed successfully", order_id);
                }
            }
            _ => {
                // Ignore other events
            }
        }
        
        Ok(())
    }
}
