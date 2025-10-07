use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::error::Result;

/// Order states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrderState {
    Created,
    Processing,
    Processed,
    Confirmed,
    Failed,
}

impl fmt::Display for OrderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderState::Created => write!(f, "Created"),
            OrderState::Processing => write!(f, "Processing"),
            OrderState::Processed => write!(f, "Processed"),
            OrderState::Confirmed => write!(f, "Confirmed"),
            OrderState::Failed => write!(f, "Failed"),
        }
    }
}

/// Order data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub amount: f64,
    pub asset: String,
    pub state: OrderState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Event: New order created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub order: Order,
}

impl OrderCreatedEvent {
    pub fn new(order: Order) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            order,
        }
    }
}

/// Event: Order processing started
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderProcessingEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub order_id: Uuid,
}

impl OrderProcessingEvent {
    pub fn new(order_id: Uuid) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            order_id,
        }
    }
}

/// Event: Order processed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderProcessedEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub order_id: Uuid,
}

impl OrderProcessedEvent {
    pub fn new(order_id: Uuid) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            order_id,
        }
    }
}

/// Event: Order confirmed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConfirmedEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub order_id: Uuid,
}

impl OrderConfirmedEvent {
    pub fn new(order_id: Uuid) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            order_id,
        }
    }
}

/// Unified event enum - type-safe event representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderEvent {
    Created(OrderCreatedEvent),
    Processing(OrderProcessingEvent),
    Processed(OrderProcessedEvent),
    Confirmed(OrderConfirmedEvent),
}

impl OrderEvent {
    /// Get the event ID
    pub fn event_id(&self) -> Uuid {
        match self {
            OrderEvent::Created(e) => e.event_id,
            OrderEvent::Processing(e) => e.event_id,
            OrderEvent::Processed(e) => e.event_id,
            OrderEvent::Confirmed(e) => e.event_id,
        }
    }
    
    /// Get the event timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            OrderEvent::Created(e) => e.timestamp,
            OrderEvent::Processing(e) => e.timestamp,
            OrderEvent::Processed(e) => e.timestamp,
            OrderEvent::Confirmed(e) => e.timestamp,
        }
    }
    
    /// Get the event type name
    pub fn event_type(&self) -> &str {
        match self {
            OrderEvent::Created(_) => "OrderCreated",
            OrderEvent::Processing(_) => "OrderProcessing",
            OrderEvent::Processed(_) => "OrderProcessed",
            OrderEvent::Confirmed(_) => "OrderConfirmed",
        }
    }
}

/// Event handler trait for components
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an incoming event
    async fn handle_event(&self, event: Arc<OrderEvent>) -> Result<()>;
}

/// Event bus for pub/sub pattern
pub struct EventBus {
    sender: broadcast::Sender<Arc<OrderEvent>>,
    handlers: Arc<RwLock<Vec<Arc<dyn EventHandler>>>>,
}

impl EventBus {
    /// Create a new event bus with a channel capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Register an event handler
    pub async fn register_handler(&self, handler: Arc<dyn EventHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
    }
    
    /// Publish an event to all subscribers
    pub async fn publish(&self, event: Arc<OrderEvent>) -> Result<()> {
        // Send via broadcast channel (non-blocking)
        let _ = self.sender.send(event.clone());
        
        // Also call handlers directly (ensures delivery)
        let handlers = self.handlers.read().await;
        for handler in handlers.iter() {
            // Spawn tasks to handle events concurrently
            let handler_clone = handler.clone();
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = handler_clone.handle_event(event_clone).await {
                    tracing::error!("Handler error: {}", e);
                }
            });
        }
        
        Ok(())
    }
    
    /// Subscribe to events (returns a receiver)
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<OrderEvent>> {
        self.sender.subscribe()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            handlers: self.handlers.clone(),
        }
    }
}

impl fmt::Debug for EventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBus").finish()
    }
}
