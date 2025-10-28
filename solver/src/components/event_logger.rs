use async_trait::async_trait;
use tracing::info;

use crate::error::Result;
use crate::events::{EventHandler, SolverEvent};

pub struct EventLogger {}

impl EventLogger {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl EventHandler for EventLogger {
    fn name(&self) -> &'static str {
        "EventLogger"
    }

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        match event {
            SolverEvent::OrderCreated(e) => {
                info!(
                    event = "OrderCreated",
                    order_id = %e.order_id,
                    timestamp = e.timestamp,
                );
            }
            SolverEvent::Start => {
                info!(event = "Start");
            }
            SolverEvent::Stop => {
                info!(event = "Stop");
            }
            SolverEvent::OrderFill(e) => {
                info!(
                    event = "OrderFill",
                    order_id = %e.order_id,
                    timestamp = e.timestamp,
                    amount = %e.amount,
                );
            }
            SolverEvent::OrderRejected(e) => {
                info!(
                    event = "OrderRejected",
                    order_id = %e.order_id,
                    timestamp = e.timestamp,
                    reason = %e.reason,
                );
            }
            SolverEvent::OrderCancelRequest(e) => {
                info!(
                    event = "OrderCancelRequest",
                    order_id = %e.order_id,
                    timestamp = e.timestamp,
                    new_fill_deadline = e.new_fill_deadline,
                );
            }
            SolverEvent::OrderRefundClaimed(e) => {
                info!(
                    event = "OrderRefundClaimed",
                    order_id = %e.order_id,
                    timestamp = e.timestamp,
                    sender = %e.sender,
                    amount_refunded = %e.amount_refunded,
                );
            }
            SolverEvent::OrderCompleted(e) => {
                info!(
                    event = "OrderCompleted",
                    order_id = %e.order_id,
                    timestamp = e.timestamp,
                );
            }
            SolverEvent::RequestRebalance(e) => {
                info!(
                    event = "RequestRebalance",
                    target_order_id = %e.target_order_id,
                    timestamp = e.timestamp,
                    asset = ?e.asset,
                    amount = %e.amount,
                );
            }
        }

        Ok(vec![])
    }
}
