use async_trait::async_trait;
use order_book::OrderData;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{Result, SolverError};
use crate::events::{EventHandler, SolverEvent};

#[derive(Debug, Clone)]
pub struct Order {
    pub id: String,
    pub state: OrderState,
    pub data: OrderData,
    pub filled_amount: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderState {
    Created,
    PartiallyFilled,
    Filled,
    Completed,
    Rejected,
    Cancelled,
}

impl fmt::Display for OrderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderState::Created => write!(f, "Created"),
            OrderState::PartiallyFilled => write!(f, "PartiallyFilled"),
            OrderState::Filled => write!(f, "Filled"),
            OrderState::Completed => write!(f, "Completed"),
            OrderState::Rejected => write!(f, "Rejected"),
            OrderState::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Event store for tracking order status
pub struct OrderStore {
    orders: Arc<RwLock<HashMap<String, Order>>>,
}

impl OrderStore {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_order(&self, order_id: &String) -> Result<Option<Order>> {
        let orders = self.orders.read().await;
        Ok(orders.get(order_id).cloned())
    }

    pub async fn get_all_orders(&self) -> Result<Vec<Order>> {
        let orders = self.orders.read().await;
        Ok(orders.values().cloned().collect())
    }

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
impl EventHandler for OrderStore {
    fn name(&self) -> &'static str {
        "OrderStore"
    }

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn handle_event(&self, event: Arc<SolverEvent>) -> Result<Arc<Vec<SolverEvent>>> {
        let mut orders = self.orders.write().await;

        match event.as_ref() {
            SolverEvent::Created(e) => {
                let order = Order {
                    id: e.order_id.clone(),
                    state: OrderState::Created,
                    data: e.order.clone(),
                    filled_amount: 0,
                };
                orders.insert(order.id.clone(), order);
            }
            SolverEvent::Fill(e) => {
                let order = orders
                    .get_mut(&e.order_id)
                    .ok_or_else(|| SolverError::OrderNotFound(e.order_id.to_string()))?;

                order.state = OrderState::PartiallyFilled;
                order.filled_amount += e.amount;

                if order.filled_amount >= order.data.amount_out {
                    order.state = OrderState::Filled;
                }
            }
            SolverEvent::Rejected(e) => {
                let order = orders
                    .get_mut(&e.order_id)
                    .ok_or_else(|| SolverError::OrderNotFound(e.order_id.clone()))?;

                order.state = OrderState::Rejected;
            }
            SolverEvent::CancelRequest(e) => {
                let order = orders
                    .get_mut(&e.order_id)
                    .ok_or_else(|| SolverError::OrderNotFound(e.order_id.clone()))?;

                order.state = OrderState::Cancelled;
            }
            SolverEvent::RefundClaimed(e) => {
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
            SolverEvent::Completed(e) => {
                if let Some(order) = orders.get_mut(&e.order_id) {
                    order.state = OrderState::Completed;
                }
            }
            _ => {}
        }

        Ok(Arc::new(vec![]))
    }
}
