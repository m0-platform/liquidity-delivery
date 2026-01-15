use async_trait::async_trait;
use slog::{info, Logger};

use crate::error::Result;
use crate::events::{EventHandler, SolverEvent};

pub struct EventLogger {
    logger: Logger,
}

impl EventLogger {
    pub fn new(logger: Logger) -> Self {
        Self { logger }
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
                    self.logger,
                    "OrderCreated";
                    "order_id" => %e.order_id,
                    "from_asset" => hex::encode(e.order.token_in),
                    "to_asset" => hex::encode(e.order.token_out),
                    "amount" => %e.order.amount_out,
                    "from_chain" => %e.order.origin_chain_id,
                    "to_chain" => %e.order.dest_chain_id,
                );
            }
            SolverEvent::Start => {
                info!(self.logger, "Start");
            }
            SolverEvent::Stop => {
                info!(self.logger, "Stop");
            }
            SolverEvent::OrderFill(e) => {
                info!(
                    self.logger,
                    "OrderFill";
                    "order_id" => %e.order_id,
                    "timestamp" => e.timestamp,
                    "amount" => %e.amount,
                );
            }
            SolverEvent::OrderRejected(e) => {
                info!(
                    self.logger,
                    "OrderRejected";
                    "order_id" => %e.order_id,
                    "timestamp" => e.timestamp,
                    "reason" => %e.reason,
                );
            }
            SolverEvent::OrderCancelRequest(e) => {
                info!(
                    self.logger,
                    "OrderCancelRequest";
                    "order_id" => %e.order_id,
                    "timestamp" => e.timestamp,
                    "new_fill_deadline" => e.new_fill_deadline,
                );
            }
            SolverEvent::OrderRefundClaimed(e) => {
                info!(
                    self.logger,
                    "OrderRefundClaimed";
                    "order_id" => %e.order_id,
                    "timestamp" => e.timestamp,
                    "sender" => %e.sender,
                    "amount_refunded" => %e.amount_refunded,
                );
            }
            SolverEvent::OrderCompleted(e) => {
                info!(
                    self.logger,
                    "OrderCompleted";
                    "order_id" => %e.order_id,
                    "timestamp" => e.timestamp,
                );
            }
            SolverEvent::RequestRebalance(e) => {
                info!(
                    self.logger,
                    "RequestRebalance";
                    "target_order_id" => %e.target_order_id,
                    "timestamp" => e.timestamp,
                    "asset" => ?e.asset,
                    "amount" => %e.amount,
                );
            }
        }

        Ok(vec![])
    }
}
