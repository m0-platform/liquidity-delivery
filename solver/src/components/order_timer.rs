use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use tokio::time::{interval, Duration};

use crate::error::Result;
use crate::events::{EventHandler, SolverEvent};

const ORDER_WARNING_THRESHOLD_SECONDS: u64 = 300;

pub struct OrderTimer {
    active_orders: Arc<RwLock<HashMap<String, u64>>>,
    shutdown: Arc<RwLock<Option<oneshot::Sender<()>>>>,
}

impl OrderTimer {
    pub fn new() -> Self {
        Self {
            active_orders: Arc::new(RwLock::new(HashMap::new())),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    async fn start_poller(&self) {
        let active_orders = Arc::clone(&self.active_orders);

        let (tx, mut rx) = oneshot::channel();
        *self.shutdown.write().await = Some(tx);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();

                        let orders = active_orders.read().await;
                        for (order_id, start_time) in orders.iter() {
                            let age = now - start_time;
                            if age > ORDER_WARNING_THRESHOLD_SECONDS {
                                tracing::warn!(
                                    order_id = order_id,
                                    age = age,
                                    threshold = ORDER_WARNING_THRESHOLD_SECONDS,
                                    "Order has been active longer than warning threshold"
                                );
                            }
                        }
                    }
                    _ = &mut rx => {
                        break;
                    }
                }
            }
        });
    }

    async fn stop_poller(&self) {
        if let Some(tx) = self.shutdown.write().await.take() {
            let _ = tx.send(());
        }
    }
}

#[async_trait]
impl EventHandler for OrderTimer {
    fn name(&self) -> &'static str {
        "OrderTimer"
    }

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        match event {
            SolverEvent::OrderCreated(e) => {
                self.active_orders
                    .write()
                    .await
                    .insert(e.order_id.clone(), e.timestamp);
            }
            SolverEvent::OrderCompleted(e) => {
                let start = self.active_orders.read().await.get(&e.order_id).cloned();

                match start {
                    Some(start_time) => {
                        let duration = e.timestamp - start_time;
                        tracing::info!(
                            order_id = e.order_id,
                            duration = duration,
                            "Order completion time",
                        );
                        self.active_orders.write().await.remove(&e.order_id);
                    }
                    None => {
                        tracing::warn!(
                            order_id = e.order_id,
                            "Order completed but start time not found"
                        );
                    }
                }
            }
            SolverEvent::Start => {
                self.start_poller().await;
            }
            SolverEvent::Stop => {
                self.stop_poller().await;
            }
            _ => {}
        }

        Ok(vec![])
    }
}
