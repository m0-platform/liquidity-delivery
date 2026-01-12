use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tracked order from OrderOpened event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedOrder {
    pub order_id: String,
    pub origin_chain_id: u32,
    pub sender: String,
    pub token_in: String,
    pub amount_in: String,
    pub dest_chain_id: u32,
    pub token_out: String,
    pub amount_out: String,
    pub solver: String,
}

#[derive(Clone)]
pub struct OrderStore {
    orders: Arc<RwLock<HashMap<String, TrackedOrder>>>,
    orders_by_sender: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl OrderStore {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
            orders_by_sender: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn insert_order(&self, order: TrackedOrder) {
        let order_id = order.order_id.clone();
        let sender = order.sender.clone().to_lowercase();

        let mut orders = self.orders.write().await;
        orders.insert(order_id.clone(), order);

        let mut by_sender = self.orders_by_sender.write().await;
        by_sender.entry(sender).or_default().push(order_id);
    }

    pub async fn get_all_orders(&self) -> Vec<TrackedOrder> {
        let orders = self.orders.read().await;
        orders.values().cloned().collect()
    }

    pub async fn get_orders_by_sender(&self, sender: &str) -> Vec<TrackedOrder> {
        let sender_lower = sender.to_lowercase();
        let by_sender = self.orders_by_sender.read().await;
        let orders = self.orders.read().await;

        by_sender
            .get(&sender_lower)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| orders.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub async fn get_order(&self, order_id: &str) -> Option<TrackedOrder> {
        let orders = self.orders.read().await;
        orders.get(order_id).cloned()
    }

    pub async fn has_order(&self, order_id: &str) -> bool {
        let orders = self.orders.read().await;
        orders.contains_key(order_id)
    }
}

impl Default for OrderStore {
    fn default() -> Self {
        Self::new()
    }
}
