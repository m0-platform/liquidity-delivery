use async_trait::async_trait;
use m0_liquidity_sdk::types::Asset;
use slog::{info, Logger};
use std::cmp::min;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::components::ComponentParams;
use crate::config::SupportedAssets;
use crate::error::Result;
use crate::events::{
    EventHandler, EventProcessor, OrderRejectEvent, RequestFillOrderEvent, RequestHoldEvent,
    SolverEvent,
};
use crate::stores::{AssetStore, OrderStore};

pub struct OrderProcessor {
    order_store: Arc<RwLock<OrderStore>>,
    asset_store: Arc<RwLock<AssetStore>>,
    supported_assets: SupportedAssets,
    logger: Logger,
    max_clip_size: u128,
    fee_bps: u128,
}

impl OrderProcessor {
    pub fn new(params: &ComponentParams) -> Self {
        Self {
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            asset_store: Arc::new(RwLock::new(AssetStore::new(
                params.config.liquidity_api_url.clone(),
            ))),
            supported_assets: params.config.supported_assets.clone(),
            logger: params.logger.new(slog::o!("component" => "OrderProcessor")),
            max_clip_size: params.config.max_order_clip_size as u128,
            fee_bps: params.config.solver_fee_bps as u128,
        }
    }

    async fn get_supported_asset(
        &self,
        token_address: [u8; 32],
        chain_id: u32,
    ) -> std::result::Result<Asset, String> {
        let asset_store = self.asset_store.read().await;

        let asset = asset_store
            .get_asset(token_address, chain_id)
            .await
            .ok_or_else(|| "Asset not supported".to_string())?;

        if !self.supported_assets.is_asset_supported(&asset) {
            return Err(format!("Asset {} not supported", asset.address));
        }

        Ok(asset)
    }
}

#[async_trait]
impl EventHandler for OrderProcessor {
    fn name(&self) -> &'static str {
        "OrderProcessor"
    }

    async fn initialize(&self) -> Result<()> {
        self.order_store.write().await.initialize().await?;
        self.asset_store.write().await.initialize().await?;
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let store = self.order_store.read().await;
        let _ = store.handle_event(event.clone()).await;

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
                let fill_amount = min(e.order.amount_out, max_size);

                let notional = e.order.amount_out / 10u128.pow(destination_asset.decimals as u32);
                if notional > 100_000 {
                    info!(
                        self.logger,
                        "Received large order";
                        "order_id" => e.order_id.clone(),
                        "notional" => notional
                    );
                }

                // Request hold on destination asset
                return Ok(vec![SolverEvent::RequestHold(RequestHoldEvent::new(
                    e.order_id,
                    destination_asset,
                    fill_amount,
                ))]);
            }
            SolverEvent::HoldSuccessful(e) => {
                return Ok(vec![SolverEvent::RequestFillOrder(
                    RequestFillOrderEvent::new(e.order_id, e.hold_amount),
                )]);
            }
            _ => {}
        }

        Ok(vec![])
    }
}
