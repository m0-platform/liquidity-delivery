use async_trait::async_trait;
use slog::{info, warn, Logger};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use tokio::time::{interval, Duration};

use crate::components::ComponentParams;
use crate::error::Result;
use crate::events::{EventHandler, SolverEvent};

const ORDER_WARNING_THRESHOLD_SECONDS: u64 = 7200;

pub struct OrderTimer {
    active_orders: Arc<RwLock<HashMap<String, u64>>>,
    shutdown: Arc<RwLock<Option<oneshot::Sender<()>>>>,
    logger: Logger,
}

impl OrderTimer {
    pub fn new(params: &ComponentParams) -> Self {
        Self {
            active_orders: Arc::new(RwLock::new(HashMap::new())),
            shutdown: Arc::new(RwLock::new(None)),
            logger: params.logger.new(slog::o!("component" => "OrderTimer")),
        }
    }

    async fn start_poller(&self) {
        let active_orders = Arc::clone(&self.active_orders);
        let logger = self.logger.clone();

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
                                warn!(
                                    logger,
                                    "Order has been active longer than warning threshold";
                                    "order_id" => order_id,
                                    "age" => age,
                                    "threshold" => ORDER_WARNING_THRESHOLD_SECONDS,
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
                        info!(
                            self.logger,
                            "Order completion time";
                            "order_id" => &e.order_id,
                            "duration" => duration,
                        );
                        self.active_orders.write().await.remove(&e.order_id);
                    }
                    None => {
                        warn!(
                            self.logger,
                            "Order completed but start time not found";
                            "order_id" => &e.order_id,
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
