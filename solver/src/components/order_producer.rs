use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::components::Component;
use crate::error::Result;
use crate::events::{EventBus, Order, OrderState, OrderCreatedEvent, OrderEvent};

/// Component that produces new order events
pub struct OrderProducer {
    running: Arc<RwLock<bool>>,
}

impl OrderProducer {
    pub fn new() -> Self {
        Self {
            running: Arc::new(RwLock::new(false)),
        }
    }
}

impl Default for OrderProducer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Component for OrderProducer {
    fn name(&self) -> &str {
        "OrderProducer"
    }
    
    async fn initialize(&self) -> Result<()> {
        tracing::info!("OrderProducer: Initializing");
        Ok(())
    }
    
    async fn start(&self, event_bus: Arc<EventBus>) -> Result<()> {
        tracing::info!("OrderProducer: Starting");
        
        let mut running = self.running.write().await;
        *running = true;
        drop(running);
        
        let running_clone = self.running.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
            
            loop {
                interval.tick().await;
                
                let is_running = *running_clone.read().await;
                if !is_running {
                    break;
                }
                
                // Create a new order
                let order = Order {
                    id: Uuid::new_v4(),
                    amount: 100.0 + (rand::random::<f64>() * 900.0),
                    asset: "USD".to_string(),
                    state: OrderState::Created,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };
                
                let event = OrderCreatedEvent::new(order.clone());
                tracing::info!("OrderProducer: Creating order {} with amount ${:.2}", order.id, order.amount);
                
                if let Err(e) = event_bus.publish(Arc::new(OrderEvent::Created(event))).await {
                    tracing::error!("OrderProducer: Failed to publish event: {}", e);
                }
            }
            
            tracing::info!("OrderProducer: Stopped");
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
