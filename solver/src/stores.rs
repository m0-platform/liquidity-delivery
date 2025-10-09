use async_trait::async_trait;
use order_book::OrderData;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{Result, SolverError};
use crate::events::{EventHandler, OrderEvent, OrderState};

/// Base trait for all stores
pub trait Store: Send + Sync + EventHandler {
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: String,
    pub state: OrderState,
    pub data: OrderData,
}

/// Event store for tracking order states
pub struct OrderStore {
    orders: Arc<RwLock<HashMap<String, Order>>>,
}

impl OrderStore {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get an order by ID
    pub async fn get_order(&self, order_id: &String) -> Result<Option<Order>> {
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
}

#[async_trait]
impl Store for OrderStore {
    fn name(&self) -> &str {
        "OrderStore"
    }
}

#[async_trait]
impl EventHandler for OrderStore {
    async fn handle_event(&self, event: Arc<OrderEvent>) -> Result<()> {
        let mut orders = self.orders.write().await;

        match event.as_ref() {
            OrderEvent::Created(e) => {
                let order = Order {
                    id: e.order_id.clone(),
                    state: OrderState::Created,
                    data: e.order.clone(),
                };
                orders.insert(order.id.clone(), order);
            }
            OrderEvent::Fill(e) => {
                let order = orders
                    .get_mut(&e.order_id)
                    .ok_or_else(|| SolverError::OrderNotFound(e.order_id.to_string()))?;

                order.state = OrderState::PartiallyFilled;
            }
            OrderEvent::Rejected(e) => {
                let order = orders
                    .get_mut(&e.order_id)
                    .ok_or_else(|| SolverError::OrderNotFound(e.order_id.clone()))?;

                order.state = OrderState::Rejected;
            }
            OrderEvent::CancelRequest(e) => {
                // Order cancellation has been requested
                // We keep the order in the store but could add a "Cancelling" state if needed
                tracing::info!("Cancel requested for order {}", e.order_id);
            }
            OrderEvent::RefundClaimed(e) => {
                // Refund has been claimed for an unfilled order
                // The order should be marked as complete/failed
                if let Some(order) = orders.get_mut(&e.order_id) {
                    order.state = OrderState::Rejected;
                }
                tracing::info!(
                    "Refund claimed for order {}: {} refunded to {}",
                    e.order_id,
                    e.amount_refunded,
                    e.sender
                );
            }
            OrderEvent::Completed(e) => {
                // Order has been fully completed
                if let Some(order) = orders.get_mut(&e.order_id) {
                    order.state = OrderState::Filled;
                }
                tracing::info!("Order {} completed", e.order_id);
            }
        }

        Ok(())
    }
}
