use alloy::primitives::Address;
use async_trait::async_trait;
use slog::{error, info, Logger};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::components::ComponentParams;
use crate::config::{ChainConfig, Signers};
use crate::contracts::IOrderBook;
use crate::error::Result;
use crate::events::{EventHandler, EventProcessor, FillOrderSuccessfulEvent, SolverEvent};
use crate::providers::ProviderManager;
use crate::stores::OrderStore;
use crate::utils::{decode_evm_address, decode_order_id};

pub struct EvmWriter {
    signers: Signers,
    order_store: Arc<RwLock<OrderStore>>,
    provider_manager: Arc<ProviderManager>,
    chains: Vec<ChainConfig>,
    logger: Logger,
}

impl EvmWriter {
    pub fn new(params: &ComponentParams) -> Self {
        Self {
            signers: params.config.signers.clone(),
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            provider_manager: params.provider_manager.clone(),
            chains: params.config.chains.clone(),
            logger: params.logger.new(slog::o!("component" => "EvmWriter")),
        }
    }

    fn get_order_book_address(&self, chain_id: u32) -> Option<String> {
        self.chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .map(|c| c.order_book_address.clone())
    }
}

#[async_trait]
impl EventHandler for EvmWriter {
    fn name(&self) -> &'static str {
        "EvmWriter"
    }

    async fn initialize(&self) -> Result<()> {
        self.order_store.write().await.initialize().await?;

        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let store = self.order_store.read().await;
        let _ = store.handle_event(event.clone()).await;

        match event {
            SolverEvent::RequestFillOrder(e) => {
                let order = store.get_order(&e.order_id).await.unwrap();
                let dest_chain_id = order.data.dest_chain_id;

                let order_book_address_str = self.get_order_book_address(dest_chain_id).unwrap();
                let order_book_address = Address::from_str(&order_book_address_str).unwrap();
                let provider_wrapper = self
                    .provider_manager
                    .get_evm_provider(dest_chain_id)
                    .await
                    .unwrap();

                let order_id_bytes = decode_order_id(&e.order_id);
                let solver_address = self.signers.evm_address();

                let order_data = IOrderBook::OrderData {
                    version: order.data.version,
                    sender: order.data.sender.into(),
                    nonce: order.data.nonce,
                    originChainId: order.data.origin_chain_id,
                    destChainId: order.data.dest_chain_id,
                    fillDeadline: order.data.fill_deadline,
                    tokenIn: order.data.token_in.into(),
                    tokenOut: order.data.token_out.into(),
                    amountIn: order.data.amount_in,
                    amountOut: order.data.amount_out,
                    recipient: order.data.recipient.into(),
                    solver: order.data.solver.into(),
                };

                let fill_params = IOrderBook::FillParams {
                    amountOutToFill: e.amount,
                    originRecipient: decode_evm_address(solver_address).into(),
                };

                let provider = provider_wrapper.provider().await;
                let order_book = IOrderBook::new(order_book_address, provider);

                // Call fillOrder
                match order_book
                    .fillOrder(order_id_bytes.into(), order_data, fill_params)
                    .send()
                    .await
                {
                    Ok(pending_tx) => {
                        info!(
                            self.logger,
                            "Fill order transaction submitted";
                            "order_id" => %e.order_id,
                            "tx_hash" => %pending_tx.tx_hash(),
                        );

                        // Wait for the transaction to be mined
                        match pending_tx.get_receipt().await {
                            Ok(receipt) => {
                                info!(
                                    self.logger,
                                    "Fill order transaction confirmed";
                                    "order_id" => %e.order_id,
                                    "tx_hash" => %receipt.transaction_hash,
                                    "block_number" => ?receipt.block_number,
                                );

                                // Return success event
                                return Ok(vec![SolverEvent::FillOrderSuccessful(
                                    FillOrderSuccessfulEvent::new(e.order_id),
                                )]);
                            }
                            Err(err) => {
                                error!(
                                    self.logger,
                                    "Failed to get transaction receipt";
                                    "order_id" => %e.order_id,
                                    "error" => %err,
                                );
                            }
                        }
                    }
                    Err(err) => {
                        error!(
                            self.logger,
                            "Failed to submit fill order transaction";
                            "order_id" => %e.order_id,
                            "error" => %err,
                        );
                    }
                }
            }
            _ => {}
        }

        Ok(vec![])
    }
}
