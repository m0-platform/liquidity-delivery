use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{Result, SolverError};
use crate::events::{
    EventHandler, Order, OrderState, OrderEvent,
};

/// Base trait for all stores
#[async_trait]
pub trait Store: Send + Sync {
    /// Initialize the store
    async fn initialize(&self) -> Result<()>;
    
    /// Get the store name for logging
    fn name(&self) -> &str;
}

/// Event store for tracking order states
pub struct EventStore {
    orders: Arc<RwLock<HashMap<Uuid, Order>>>,
}

impl EventStore {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Get an order by ID
    pub async fn get_order(&self, order_id: &Uuid) -> Result<Option<Order>> {
        let orders = self.orders.read().await;
        Ok(orders.get(order_id).cloned())
    }
    
    /// Get all orders
    pub async fn get_all_orders(&self) -> Result<Vec<Order>> {
        let orders = self.orders.read().await;
        Ok(orders.values().cloned().collect())
    }
    
    /// Get orders by state
    pub async fn get_orders_by_state(&self, state: OrderState) -> Result<Vec<Order>> {
        let orders = self.orders.read().await;
        Ok(orders
            .values()
            .filter(|o| o.state == state)
            .cloned()
            .collect())
    }
    
    /// Create a new order
    async fn create_order(&self, order: Order) -> Result<()> {
        let mut orders = self.orders.write().await;
        orders.insert(order.id, order);
        Ok(())
    }
    
    /// Update order state
    async fn update_order_state(&self, order_id: &Uuid, new_state: OrderState) -> Result<()> {
        let mut orders = self.orders.write().await;
        
        let order = orders
            .get_mut(order_id)
            .ok_or_else(|| SolverError::OrderNotFound(order_id.to_string()))?;
        
        // Validate state transition
        self.validate_state_transition(&order.state, &new_state)?;
        
        order.state = new_state;
        order.updated_at = chrono::Utc::now();
        
        Ok(())
    }
    
    /// Validate state transitions
    fn validate_state_transition(&self, from: &OrderState, to: &OrderState) -> Result<()> {
        let valid = match (from, to) {
            (OrderState::Created, OrderState::Processing) => true,
            (OrderState::Processing, OrderState::Processed) => true,
            (OrderState::Processing, OrderState::Failed) => true,
            (OrderState::Processed, OrderState::Confirmed) => true,
            (OrderState::Processed, OrderState::Failed) => true,
            _ => false,
        };
        
        if !valid {
            return Err(SolverError::InvalidStateTransition {
                from: from.to_string(),
                to: to.to_string(),
            });
        }
        
        Ok(())
    }
    
    /// Get count of orders in each state
    pub async fn get_state_counts(&self) -> Result<HashMap<String, usize>> {
        let orders = self.orders.read().await;
        let mut counts = HashMap::new();
        
        for order in orders.values() {
            *counts.entry(order.state.to_string()).or_insert(0) += 1;
        }
        
        Ok(counts)
    }
}

#[async_trait]
impl Store for EventStore {
    async fn initialize(&self) -> Result<()> {
        tracing::info!("Initializing EventStore");
        Ok(())
    }
    
    fn name(&self) -> &str {
        "EventStore"
    }
}

/// EventStore also implements EventHandler to update state based on events
#[async_trait]
impl EventHandler for EventStore {
    async fn handle_event(&self, event: Arc<OrderEvent>) -> Result<()> {
        match event.as_ref() {
            OrderEvent::Created(e) => {
                tracing::info!("EventStore: Creating order {}", e.order.id);
                self.create_order(e.order.clone()).await?;
            }
            OrderEvent::Processing(e) => {
                tracing::info!("EventStore: Updating order {} to Processing", e.order_id);
                self.update_order_state(&e.order_id, OrderState::Processing).await?;
            }
            OrderEvent::Processed(e) => {
                tracing::info!("EventStore: Updating order {} to Processed", e.order_id);
                self.update_order_state(&e.order_id, OrderState::Processed).await?;
            }
            OrderEvent::Confirmed(e) => {
                tracing::info!("EventStore: Updating order {} to Confirmed", e.order_id);
                self.update_order_state(&e.order_id, OrderState::Confirmed).await?;
            }
        }
        
        Ok(())
    }
}

impl Default for EventStore {
    fn default() -> Self {
        Self::new()
    }
}
