use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::components::Component;
use crate::error::Result;
use crate::events::{
    EventBus, EventHandler, OrderEvent, OrderProcessingEvent, OrderProcessedEvent,
};
use crate::stores::EventStore;

/// Component that processes orders
pub struct OrderProcessor {
    _event_store: Arc<EventStore>,
    running: Arc<RwLock<bool>>,
}

impl OrderProcessor {
    pub fn new(event_store: Arc<EventStore>) -> Self {
        Self {
            _event_store: event_store,
            running: Arc::new(RwLock::new(false)),
        }
    }
}

#[async_trait]
impl Component for OrderProcessor {
    fn name(&self) -> &str {
        "OrderProcessor"
    }
    
    async fn initialize(&self) -> Result<()> {
        tracing::info!("OrderProcessor: Initializing");
        Ok(())
    }
    
    async fn start(&self, event_bus: Arc<EventBus>) -> Result<()> {
        tracing::info!("OrderProcessor: Starting");
        
        let mut running = self.running.write().await;
        *running = true;
        drop(running);
        
        // Register as event handler
        let handler = Arc::new(OrderProcessorHandler {
            event_bus: event_bus.clone(),
        });
        
        event_bus.register_handler(handler).await;
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        tracing::info!("OrderProcessor: Stopping");
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }
}

/// Handler for processing order events
struct OrderProcessorHandler {
    event_bus: Arc<EventBus>,
}

#[async_trait]
impl EventHandler for OrderProcessorHandler {
    async fn handle_event(&self, event: Arc<OrderEvent>) -> Result<()> {
        match event.as_ref() {
            OrderEvent::Created(e) => {
                let order_id = e.order.id;
                
                tracing::info!(
                    "OrderProcessor: Processing order {} (amount: ${:.2})",
                    order_id,
                    e.order.amount
                );
                
                // Simulate processing delay
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                // Publish processing started event
                let processing_event = OrderProcessingEvent::new(order_id);
                self.event_bus.publish(Arc::new(OrderEvent::Processing(processing_event))).await?;
                
                // Simulate more processing
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                // Publish processed event
                let processed_event = OrderProcessedEvent::new(order_id);
                self.event_bus.publish(Arc::new(OrderEvent::Processed(processed_event))).await?;
                
                tracing::info!("OrderProcessor: Completed processing order {}", order_id);
            }
            _ => {
                // Ignore other events
            }
        }
        
        Ok(())
    }
}
