use alloy::primitives::{Address, U256};
use async_trait::async_trait;
use m0_liquidity_sdk::types::ChainRuntime;
use slog::{error, info, warn, Logger};
use std::str::FromStr;
use std::sync::Arc;

use crate::components::ComponentParams;
use crate::config::ChainConfig;
use crate::contracts::{IOrderBook, IERC20};
use crate::error::{Result, SolverError};
use crate::events::{EventHandler, EventProcessor, FillOrderSuccessfulEvent, SolverEvent};
use crate::providers::ProviderManager;
use crate::stores::OrderStore;
use crate::utils::{chain_runtime, decode_evm_address, decode_order_id, encode_evm_address};

pub struct EvmWriter {
    order_store: Arc<OrderStore>,
    provider_manager: Arc<ProviderManager>,
    chains: Vec<ChainConfig>,
    logger: Logger,
}

impl EvmWriter {
    pub fn new(params: &ComponentParams) -> Self {
        Self {
            order_store: Arc::new(OrderStore::new()),
            provider_manager: params.provider_manager.clone(),
            chains: params.config.chains.clone(),
            logger: params.logger.new(slog::o!("component" => "EvmWriter")),
        }
    }

    fn get_order_book_address(&self, chain_id: u32) -> Result<Address> {
        self.chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .map(|c| Address::from_str(&c.order_book_address).unwrap())
            .ok_or_else(|| {
                SolverError::Component("Order book address not found for chain".to_string())
            })
    }

    async fn approve_spending(&self, token: &[u8; 32], chain_id: u32, amount: u128) -> Result<()> {
        let provider_wrapper = self.provider_manager.get_evm_provider(chain_id).await?;
        let provider = provider_wrapper.provider().await;

        let token = encode_evm_address(token);
        let token_contract = IERC20::new(token, provider);
        let solver_address = self.provider_manager.evm_address;
        let order_book_address = self.get_order_book_address(chain_id)?;

        let current_allowance = token_contract
            .allowance(solver_address, order_book_address)
            .call()
            .await
            .map_err(|err| {
                SolverError::Component(format!("Failed to check spending approvals: {}", err))
            })?
            .to::<u128>();

        if current_allowance < amount {
            let pending_tx = token_contract
                .approve(order_book_address, U256::from(amount))
                .send()
                .await
                .map_err(|err| {
                    SolverError::Component(format!("Failed to submit approval: {}", err))
                })?;

            let tx_hash = *pending_tx.tx_hash();

            if let Err(err) = pending_tx.get_receipt().await {
                warn!(
                    self.logger,
                    "Failed to get approval transaction receipt";
                    "tx_hash" => %tx_hash,
                    "error" => %err,
                );
            }
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandler for EvmWriter {
    fn name(&self) -> &'static str {
        "EvmWriter"
    }

    async fn initialize(&self) -> Result<()> {
        self.order_store.initialize().await?;

        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let _ = self.order_store.handle_event(event.clone()).await;

        match event {
            SolverEvent::RequestFillOrder(e) => {
                let order = self.order_store.get_order(&e.order_id).await?;
                let dest_chain_id = order.data.dest_chain_id;

                if chain_runtime(dest_chain_id) != ChainRuntime::Evm {
                    return Ok(vec![]);
                }

                let order_book_address = self.get_order_book_address(dest_chain_id)?;
                let provider_wrapper = self
                    .provider_manager
                    .get_evm_provider(dest_chain_id)
                    .await?;

                let order_id_bytes = decode_order_id(&e.order_id);
                let solver_address = self.provider_manager.evm_address;

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

                // Ensure spending is approved
                self.approve_spending(&order.data.token_out, dest_chain_id, e.amount)
                    .await?;

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
