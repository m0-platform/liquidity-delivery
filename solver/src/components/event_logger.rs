use async_trait::async_trait;
use slog::{info, Logger};

use crate::components::ComponentParams;
use crate::error::Result;
use crate::events::{EventHandler, SolverEvent};

pub struct EventLogger {
    logger: Logger,
}

impl EventLogger {
    pub fn new(params: &ComponentParams) -> Self {
        let logger = params.logger.new(slog::o!("component" => "EventLogger"));
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
            SolverEvent::Start => {
                info!(self.logger, "Start");
            }
            SolverEvent::Stop => {
                info!(self.logger, "Stop");
            }
            SolverEvent::OrderCreated(e) => {
                info!(
                    self.logger,
                    "OrderCreated";
                    "order_id" => %e.order_id,
                );
            }
            SolverEvent::OrderFill(e) => {
                info!(
                    self.logger,
                    "OrderFill";
                    "order_id" => %e.order_id,
                    "amount" => %e.amount,
                );
            }
            SolverEvent::OrderRejected(e) => {
                info!(
                    self.logger,
                    "OrderRejected";
                    "order_id" => %e.order_id,
                    "reason" => %e.reason,
                );
            }
            SolverEvent::OrderCancelRequest(e) => {
                info!(
                    self.logger,
                    "OrderCancelRequest";
                    "order_id" => %e.order_id,
                    "requested_at" => e.requested_at,
                );
            }
            SolverEvent::OrderRefundClaimed(e) => {
                info!(
                    self.logger,
                    "OrderRefundClaimed";
                    "order_id" => %e.order_id,
                    "sender" => %e.sender,
                    "amount_refunded" => %e.amount_refunded,
                );
            }
            SolverEvent::OrderCompleted(e) => {
                info!(
                    self.logger,
                    "OrderCompleted";
                    "order_id" => %e.order_id,
                );
            }
            SolverEvent::RequestHold(e) => {
                info!(
                    self.logger,
                    "RequestRebalance";
                    "order_id" => %e.order_id,
                    "asset" => ?e.asset,
                    "amount" => %e.amount,
                );
            }
            SolverEvent::HoldSuccessful(e) => {
                info!(
                    self.logger,
                    "HoldSuccessful";
                    "order_id" => %e.order_id,
                );
            }
            SolverEvent::RequestFillOrder(e) => {
                info!(
                    self.logger,
                    "RequestFillOrder";
                    "order_id" => %e.order_id,
                    "fill_amount" => %e.amount,
                );
            }
            SolverEvent::FillOrderSuccessful(e) => {
                info!(
                    self.logger,
                    "FillOrderSuccessful";
                    "order_id" => %e.order_id,
                );
            }
        }

        Ok(vec![])
    }
}
