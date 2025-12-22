use async_trait::async_trait;
use m0_liquidity_sdk::types::Asset;
use slog::{info, Logger};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::api::QuoteResponse;
use crate::components::ComponentParams;
use crate::config::SupportedAssets;
use crate::error::Result;
use crate::events::{
    APIQuoteResponseEvent, EventHandler, EventProcessor, OrderRejectEvent, RequestFillOrderEvent,
    RequestHoldEvent, SolverEvent,
};
use crate::stores::{AssetStore, OrderStore};
use crate::utils::decode_address;

pub struct OrderProcessor {
    order_store: Arc<OrderStore>,
    asset_store: Arc<AssetStore>,
    supported_assets: SupportedAssets,
    logger: Logger,
    max_clip_size: u128,
    clip_process_delay: u64,
    fee_bps: u128,
    proprocess_queue: Arc<RwLock<Vec<(String, u128)>>>,
}

impl OrderProcessor {
    pub fn new(params: &ComponentParams) -> Self {
        Self {
            order_store: Arc::new(OrderStore::new()),
            asset_store: Arc::new(AssetStore::new(params.config.liquidity_api_url.clone())),
            supported_assets: params.config.supported_assets.clone(),
            logger: params.logger.new(slog::o!("component" => "OrderProcessor")),
            max_clip_size: params.config.max_order_clip_size as u128,
            clip_process_delay: params.config.max_clip_reprocess_delay_sec,
            fee_bps: params.config.solver_fee_bps as u128,
            proprocess_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn get_supported_asset(
        &self,
        token_address: [u8; 32],
        chain_id: u32,
    ) -> std::result::Result<Asset, String> {
        let asset = self
            .asset_store
            .get_asset(token_address, chain_id)
            .await
            .ok_or_else(|| "Asset not supported".to_string())?;

        if !self.supported_assets.is_asset_supported(&asset) {
            return Err(format!("Asset {} not supported", asset.address));
        }

        Ok(asset)
    }

    fn get_reprocess_time(&self) -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            + self.clip_process_delay as u128
    }
}

#[async_trait]
impl EventHandler for OrderProcessor {
    fn name(&self) -> &'static str {
        "OrderProcessor"
    }

    async fn initialize(&self) -> Result<()> {
        self.order_store.initialize().await?;
        self.asset_store.initialize().await?;
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let _ = self.order_store.handle_event(event.clone()).await;

        match event {
            SolverEvent::OrderCreated(e) => {
                if let Err(reason) = self
                    .get_supported_asset(e.order.token_in, e.order.origin_chain_id)
                    .await
                {
                    return Ok(vec![SolverEvent::OrderRejected(OrderRejectEvent::new(
                        e.order_id, reason,
                    ))]);
                }

                let destination_asset = match self
                    .get_supported_asset(e.order.token_out, e.order.dest_chain_id)
                    .await
                {
                    Ok(asset) => asset,
                    Err(reason) => {
                        return Ok(vec![SolverEvent::OrderRejected(OrderRejectEvent::new(
                            e.order_id, reason,
                        ))]);
                    }
                };

                // Make sure amount_out covers our fee
                let min_amount_out = e.order.amount_in * (10_000 - self.fee_bps) / 10_000;
                if min_amount_out < e.order.amount_out {
                    return Ok(vec![SolverEvent::OrderRejected(OrderRejectEvent::new(
                        e.order_id,
                        format!(
                            "Order amount_out {} does not cover fee-inclusive amount_out {}",
                            e.order.amount_out, min_amount_out
                        ),
                    ))]);
                }

                // Clip large orders
                let max_size = self.max_clip_size * 10u128.pow(destination_asset.decimals as u32);

                let fill_amount = if e.order.amount_out > max_size {
                    let shifted_amount =
                        e.order.amount_out / 10u128.pow(destination_asset.decimals as u32);

                    info!(
                        self.logger,
                        "Clipping large order";
                        "order_id" => e.order_id.clone(),
                        "max_clip" => self.max_clip_size,
                        "order_sizel" => shifted_amount
                    );

                    // Queue for reprocessing
                    let mut queue = self.proprocess_queue.write().await;
                    queue.push((e.order_id.clone(), self.get_reprocess_time()));

                    max_size
                } else {
                    e.order.amount_out
                };

                // Request hold on destination asset
                return Ok(vec![SolverEvent::RequestHold(RequestHoldEvent::new(
                    e.order_id,
                    destination_asset,
                    fill_amount,
                    true,
                ))]);
            }
            SolverEvent::HoldSuccessful(e) => {
                return Ok(vec![SolverEvent::RequestFillOrder(
                    RequestFillOrderEvent::new(e.order_id, e.hold_amount),
                )]);
            }
            SolverEvent::Heartbeat(ts) => {
                let mut queue = self.proprocess_queue.write().await;

                let mut events = Vec::new();
                let mut requeue = Vec::new();

                for (order_id, process_time) in queue.iter() {
                    if *process_time > ts {
                        break;
                    }

                    let order = self.order_store.get_order(order_id).await?;

                    let dest_asset = self
                        .asset_store
                        .get_asset(order.data.token_out, order.data.dest_chain_id)
                        .await
                        .unwrap();

                    let remaining = order.data.amount_out - order.filled_amount;
                    let max_size = self.max_clip_size * 10u128.pow(dest_asset.decimals as u32);

                    let fill_amount = if remaining > max_size {
                        requeue.push(order.id.clone());

                        max_size
                    } else {
                        remaining
                    };

                    events.push(SolverEvent::RequestHold(RequestHoldEvent::new(
                        order_id.clone(),
                        dest_asset,
                        fill_amount,
                        true,
                    )));
                }

                queue.drain(0..events.len());

                for order_id in requeue {
                    queue.push((order_id, self.get_reprocess_time()));
                }

                return Ok(events);
            }
            SolverEvent::APIRequestQuote(request_event) => {
                let req = request_event.request;
                let mut resp = APIQuoteResponseEvent {
                    id: request_event.id,
                    response: QuoteResponse::default(),
                };

                let input_token = match decode_address(req.input_token, req.input_chain_id) {
                    Some(asset) => asset,
                    None => {
                        resp.response.reason = Some("Invalid input token".to_string());
                        return Ok(vec![SolverEvent::APIQuoteResponse(resp)]);
                    }
                };

                let output_token = match decode_address(req.output_token, req.output_chain_id) {
                    Some(asset) => asset,
                    None => {
                        resp.response.reason = Some("Invalid output token".to_string());
                        return Ok(vec![SolverEvent::APIQuoteResponse(resp)]);
                    }
                };

                if let Err(_) = self
                    .get_supported_asset(input_token, req.input_chain_id)
                    .await
                {
                    resp.response.reason = Some("Input token not supported".to_string());
                    return Ok(vec![SolverEvent::APIQuoteResponse(resp)]);
                }

                if let Err(_) = self
                    .get_supported_asset(output_token, req.output_chain_id)
                    .await
                {
                    resp.response.reason = Some("Output token not supported".to_string());
                    return Ok(vec![SolverEvent::APIQuoteResponse(resp)]);
                }

                // Make sure amount_out covers our fee
                let min_amount_out = req.amount_in as u128 * (10_000 - self.fee_bps) / 10_000;
                if min_amount_out < req.amount_out as u128 {
                    resp.response.reason = Some("Output amount too low".to_string());
                    return Ok(vec![SolverEvent::APIQuoteResponse(resp)]);
                }

                // Solver is willing to fill the order
                resp.response.can_process = true;
                resp.response.estimated_time_seconds = 300; // 5 minutes

                return Ok(vec![SolverEvent::APIQuoteResponse(resp)]);
            }
            _ => {}
        }

        Ok(vec![])
    }
}
