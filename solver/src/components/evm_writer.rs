use alloy::primitives::{Address, Bytes, U256};
use async_trait::async_trait;
use m0_liquidity_sdk::types::ChainRuntime;
use m0_portal_common::get_wormhole_chain_id;
use serde::{Deserialize, Serialize};
use slog::{error, info, warn, Logger};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::components::ComponentParams;
use crate::config::{self, ChainConfig};
use crate::contracts::{IOrderBook, IPortal, IERC20};
use crate::error::{Result, SolverError};
use crate::events::{EventHandler, EventProcessor, FillOrderSuccessfulEvent, SolverEvent};
use crate::providers::ProviderManager;
use crate::stores::OrderStore;
use crate::utils::{chain_runtime, decode_evm_address, decode_order_id, encode_evm_address};

/// Wormhole executor quote API
const EXECUTOR_QUOTE_API_TESTNET: &str = "https://executor-testnet.labsapis.com/v0/quote";
const EXECUTOR_QUOTE_API_MAINNET: &str = "https://executor.labsapis.com/v0/quote";

/// Wormhole relay consts
const GAS_INSTRUCTION_DISCRIMINANT: u8 = 1;
const DEFAULT_GAS_LIMIT: u128 = 500_000;
const DEFAULT_MSG_VALUE: u128 = 20_000_000;

/// Request body for the Wormhole executor quote API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WormholeQuoteRequest {
    src_chain: u16,
    dst_chain: u16,
    relay_instructions: String,
}

/// Response from the Wormhole executor quote API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WormholeQuoteResponse {
    signed_quote: String,
    estimated_cost: Option<String>,
}

/// Result from fetching a Wormhole executor quote
struct WormholeQuote {
    signed_quote: Vec<u8>,
    estimated_cost: U256,
}

pub struct EvmWriter {
    order_store: Arc<OrderStore>,
    provider_manager: Arc<ProviderManager>,
    chains: Vec<ChainConfig>,
    logger: Logger,
    /// Per-chain transaction locks to prevent nonce conflicts
    tx_locks: HashMap<u32, Arc<Mutex<()>>>,
    network: config::Network,
}

impl EvmWriter {
    pub fn new(params: &ComponentParams) -> Self {
        // Create per-chain transaction locks for EVM chains
        let tx_locks: HashMap<u32, Arc<Mutex<()>>> = params
            .config
            .chains
            .iter()
            .filter(|c| chain_runtime(c.chain_id) == ChainRuntime::Evm)
            .map(|c| (c.chain_id, Arc::new(Mutex::new(()))))
            .collect();

        Self {
            order_store: Arc::new(OrderStore::new()),
            provider_manager: params.provider_manager.clone(),
            chains: params.config.chains.clone(),
            logger: params.logger.new(slog::o!("component" => "EvmWriter")),
            tx_locks,
            network: params.config.network.clone(),
        }
    }

    fn get_tx_lock(&self, chain_id: u32) -> Option<Arc<Mutex<()>>> {
        self.tx_locks.get(&chain_id).cloned()
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

    fn get_portal_address(&self, chain_id: u32) -> Result<Address> {
        self.chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .map(|c| Address::from_str(&c.portal_address).unwrap())
            .ok_or_else(|| SolverError::Component("Portal address not found for chain".to_string()))
    }

    fn get_wormhole_adapter_address(&self, chain_id: u32) -> Result<Address> {
        self.chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .map(|c| Address::from_str(&c.wormhole_adapter).unwrap())
            .ok_or_else(|| {
                SolverError::Component("Wormhole adapter address not found for chain".to_string())
            })
    }

    /// Encode relay instructions for the Wormhole executor quote API
    fn encode_relay_instructions(gas_limit: u128, msg_value: u128) -> String {
        let mut data = Vec::with_capacity(33);
        data.push(GAS_INSTRUCTION_DISCRIMINANT);
        data.extend_from_slice(&gas_limit.to_be_bytes());
        data.extend_from_slice(&msg_value.to_be_bytes());
        format!("0x{}", hex::encode(data))
    }

    /// Fetch a signed quote from the Wormhole executor API
    async fn fetch_wormhole_quote(
        &self,
        src_chain_id: u32,
        dst_chain_id: u32,
    ) -> Result<WormholeQuote> {
        let src_wormhole_chain_id = get_wormhole_chain_id(src_chain_id).ok_or_else(|| {
            SolverError::Component(format!(
                "Unknown Wormhole chain ID for source chain {}",
                src_chain_id
            ))
        })?;

        let dst_wormhole_chain_id = get_wormhole_chain_id(dst_chain_id).ok_or_else(|| {
            SolverError::Component(format!(
                "Unknown Wormhole chain ID for destination chain {}",
                dst_chain_id
            ))
        })?;

        let relay_instructions =
            Self::encode_relay_instructions(DEFAULT_GAS_LIMIT, DEFAULT_MSG_VALUE);

        let request = WormholeQuoteRequest {
            src_chain: src_wormhole_chain_id,
            dst_chain: dst_wormhole_chain_id,
            relay_instructions,
        };

        let api_url = match self.network {
            config::Network::Devnet | config::Network::Local => EXECUTOR_QUOTE_API_TESTNET,
            config::Network::Mainnet => EXECUTOR_QUOTE_API_MAINNET,
        };

        let client = reqwest::Client::new();
        let response: WormholeQuoteResponse = client
            .post(api_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| SolverError::Component(format!("Failed to fetch Wormhole quote: {}", e)))?
            .json()
            .await
            .map_err(|e| {
                SolverError::Component(format!("Failed to parse Wormhole quote response: {}", e))
            })?;

        // Decode hex to bytes (strip 0x prefix if present)
        let hex_str = response
            .signed_quote
            .strip_prefix("0x")
            .unwrap_or(&response.signed_quote);
        let signed_quote = hex::decode(hex_str).map_err(|e| {
            SolverError::Component(format!("Failed to decode signed quote hex: {}", e))
        })?;

        // Parse estimated cost (defaults to 0 if not provided)
        let estimated_cost = response
            .estimated_cost
            .as_ref()
            .and_then(|c| c.parse::<u128>().ok())
            .map(U256::from)
            .unwrap_or(U256::ZERO);

        Ok(WormholeQuote {
            signed_quote,
            estimated_cost,
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

                // Wait until order's created_at timestamp before filling
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - 15; // Subtract 15 seconds to account for chain time drift
                if now < order.data.created_at as u64 {
                    let wait_secs = order.data.created_at as u64 - now;
                    info!(
                        self.logger,
                        "Waiting for order created_at timestamp";
                        "order_id" => %e.order_id,
                        "created_at" => order.data.created_at,
                        "wait_secs" => wait_secs,
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                }

                // Acquire per-chain transaction lock to prevent nonce conflicts
                let tx_lock = self.get_tx_lock(dest_chain_id).ok_or_else(|| {
                    SolverError::Component(format!(
                        "No transaction lock for chain {}",
                        dest_chain_id
                    ))
                })?;
                let _guard = tx_lock.lock().await;

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
                    createdAt: order.data.created_at,
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
                    refundAddress: decode_evm_address(solver_address).into(),
                };

                let provider = provider_wrapper.provider().await;
                let order_book = IOrderBook::new(order_book_address, provider.clone());

                // Ensure spending is approved
                self.approve_spending(&order.data.token_out, dest_chain_id, e.amount)
                    .await?;

                // Determine bridge adapter and quote based on origin chain
                let (bridge_adapter, bridge_adapter_args, msg_value) =
                    if chain_runtime(order.data.origin_chain_id) == ChainRuntime::Svm {
                        // Solana origin - use Wormhole adapter for FillReport
                        let wormhole_quote = self
                            .fetch_wormhole_quote(dest_chain_id, order.data.origin_chain_id)
                            .await?;
                        let wormhole_adapter = self.get_wormhole_adapter_address(dest_chain_id)?;

                        (
                            wormhole_adapter,
                            Bytes::from(wormhole_quote.signed_quote),
                            wormhole_quote.estimated_cost,
                        )
                    } else {
                        // EVM origin - use default adapter (Hyperlane) with Portal quote
                        let portal_address = self.get_portal_address(dest_chain_id)?;
                        let portal = IPortal::new(portal_address, provider);
                        let quote = portal
                            .quote(order.data.origin_chain_id, IPortal::PayloadType::FillReport)
                            .call()
                            .await
                            .map_err(|err| {
                                SolverError::Component(format!(
                                    "Failed to get portal quote: {}",
                                    err
                                ))
                            })?;

                        (Address::ZERO, Bytes::new(), quote)
                    };

                let fill_result = order_book
                    .fillOrder(
                        order_id_bytes.into(),
                        order_data,
                        fill_params,
                        bridge_adapter,
                        bridge_adapter_args,
                    )
                    .value(msg_value)
                    .send()
                    .await;

                match fill_result {
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
