use async_trait::async_trait;
use m0_liquidity_sdk::types::{Asset, ChainRuntime};
use slog::{info, Logger};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::components::ComponentParams;
use crate::config::SupportedAssets;
use crate::error::Result;
use crate::events::{
    EventHandler, EventProcessor, OrderRejectEvent, QuoteResponse, QuoteResponseEvent,
    RequestFillOrderEvent, RequestHoldEvent, SolverEvent,
};
use crate::stores::{AssetStore, OrderStore};
use crate::utils::chain_runtime;

pub struct OrderProcessor {
    order_store: Arc<OrderStore>,
    asset_store: Arc<AssetStore>,
    supported_assets: SupportedAssets,
    logger: Logger,
    max_clip_size: u128,
    clip_process_delay: u64,
    fee_bps: u128,
    proprocess_queue: Arc<RwLock<Vec<(String, u128)>>>,
    solver_address_evm: String,
    solver_address_svm: String,
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
            solver_address_evm: params.provider_manager.evm_address.to_string(),
            solver_address_svm: params.provider_manager.svm_address.to_string(),
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

    async fn get_supported_assets(
        &self,
        input_token: [u8; 32],
        input_chain_id: u32,
        output_token: [u8; 32],
        output_id: u32,
    ) -> std::result::Result<(Asset, Asset), String> {
        let input_asset = self
            .get_supported_asset(input_token, input_chain_id)
            .await
            .map_err(|e| format!("input_token: {}", e))?;

        let output_asset = self
            .get_supported_asset(output_token, output_id)
            .await
            .map_err(|e| format!("output_token: {}", e))?;

        if !input_asset.m0_extension && !output_asset.m0_extension {
            return Err("At least one asset must be an M0 extension".to_string());
        }

        if input_asset == output_asset {
            return Err("Must be different assets or different chains".to_string());
        }

        Ok((input_asset, output_asset))
    }

    fn get_reprocess_time(&self) -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            + (self.clip_process_delay as u128 * 1000)
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
                let (_, destination_asset) = match self
                    .get_supported_assets(
                        e.order.token_in,
                        e.order.origin_chain_id,
                        e.order.token_out,
                        e.order.dest_chain_id,
                    )
                    .await
                {
                    Ok(assets) => assets,
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
            SolverEvent::RequestQuote(request_event) => {
                let req = request_event.request;
                let mut resp = QuoteResponseEvent {
                    id: request_event.id,
                    response: QuoteResponse {
                        fee_bps: self.fee_bps as u32,
                        ..Default::default()
                    },
                };

                let (_, output_asset) = match self
                    .get_supported_assets(
                        request_event.parsed_input_token,
                        req.input_chain_id,
                        request_event.parsed_output_token,
                        req.output_chain_id,
                    )
                    .await
                {
                    Ok(assets) => assets,
                    Err(reason) => {
                        resp.response.reject_reason = Some(reason);
                        return Ok(vec![SolverEvent::QuoteResponse(resp)]);
                    }
                };

                // Solver is willing to fill the order
                resp.response.rejected = false;
                resp.response.output_amount =
                    (req.amount_in as u128 * (10_000 - self.fee_bps) / 10_000) as u64;

                // Fill time based on how many clips will be needed to fill the order
                let max_size = self.max_clip_size * 10u128.pow(output_asset.decimals as u32);
                resp.response.est_fill_time_seconds =
                    resp.response.output_amount / max_size as u64 * self.clip_process_delay;

                resp.response.solver_address =
                    if chain_runtime(req.output_chain_id) == ChainRuntime::Svm {
                        self.solver_address_svm.clone()
                    } else {
                        self.solver_address_evm.clone()
                    };

                return Ok(vec![SolverEvent::QuoteResponse(resp)]);
            }
            _ => {}
        }

        Ok(vec![])
    }
}
