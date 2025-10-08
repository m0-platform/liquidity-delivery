use async_trait::async_trait;
use order_book::OrderData;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use std::{fmt, time::SystemTime};
use tokio::sync::broadcast;

use crate::error::Result;

/// Order states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderState {
    Created,
    PartiallyFilled,
    Filled,
    Rejected,
}

impl fmt::Display for OrderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderState::Created => write!(f, "Created"),
            OrderState::PartiallyFilled => write!(f, "PartiallyFilled"),
            OrderState::Filled => write!(f, "Filled"),
            OrderState::Rejected => write!(f, "Rejected"),
        }
    }
}

/// Event: New order created
#[derive(Debug, Clone)]
pub struct OrderCreatedEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub order: OrderData,
}

impl OrderCreatedEvent {
    pub fn new(order: OrderData) -> Self {
        Self {
            order_id: hex::encode(order.compute_order_id()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            order,
        }
    }
}

/// Event: Order fill
#[derive(Debug, Clone)]
pub struct OrderFillEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub amount: u64,
}

impl OrderFillEvent {
    pub fn new(order_id: String, amount: u64) -> Self {
        Self {
            order_id,
            amount,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Event: Order fill
#[derive(Debug, Clone)]
pub struct OrderRejectEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub reason: String,
}

impl OrderRejectEvent {
    pub fn new(order_id: String, reason: String) -> Self {
        Self {
            order_id,
            reason,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Unified event enum
#[derive(Debug, Clone)]
pub enum OrderEvent {
    Created(OrderCreatedEvent),
    Fill(OrderFillEvent),
    Rejected(OrderRejectEvent),
}

impl OrderEvent {
    pub fn order_id(&self) -> String {
        match self {
            OrderEvent::Created(e) => e.order_id.clone(),
            OrderEvent::Fill(e) => e.order_id.clone(),
            OrderEvent::Rejected(e) => e.order_id.clone(),
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
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers
    pub async fn publish(&self, event: Arc<OrderEvent>) -> Result<()> {
        let _ = self.sender.send(event.clone());
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
        }
    }
}

impl fmt::Debug for EventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBus").finish()
    }
}
