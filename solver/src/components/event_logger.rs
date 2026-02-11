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
                    "transaction_hash" => %e.transaction_hash,
                );
            }
            SolverEvent::OrderFill(e) => {
                info!(
                    self.logger,
                    "OrderFill";
                    "order_id" => %e.order_id,
                    "amount" => %e.amount,
                    "transaction_hash" => %e.transaction_hash,
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
            SolverEvent::OrderCancelled(e) => {
                info!(
                    self.logger,
                    "OrderCancelled";
                    "order_id" => %e.order_id,
                    "transaction_hash" => %e.transaction_hash,
                );
            }
            SolverEvent::OrderRefundClaimed(e) => {
                info!(
                    self.logger,
                    "OrderRefundClaimed";
                    "order_id" => %e.order_id,
                    "sender" => %e.sender,
                    "amount_refunded" => %e.amount_refunded,
                    "transaction_hash" => %e.transaction_hash,
                );
            }
            SolverEvent::OrderCompleted(e) => {
                info!(
                    self.logger,
                    "OrderCompleted";
                    "order_id" => %e.order_id,
                    "transaction_hash" => %e.transaction_hash,
                );
            }
            SolverEvent::RequestHold(e) => {
                info!(
                    self.logger,
                    "RequestHold";
                    "order_id" => %e.order_id,
                    "asset" => ?e.asset.symbol,
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
            SolverEvent::RequestSwap(e) => {
                info!(
                    self.logger,
                    "RequestSwap";
                    "order_id" => %e.order_id,
                    "from_token" => %e.token_in.symbol,
                    "to_asset" => %e.token_out.symbol,
                    "amount" => %e.amount_in,
                );
            }
            _ => {}
        }

        Ok(vec![])
    }
}
