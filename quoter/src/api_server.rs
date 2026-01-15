use alloy::primitives::{Address, FixedBytes};
use alloy::providers::ProviderBuilder;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};
use slog::{error, warn, Logger};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::config::{Asset, ChainConfig, ChainType};
use crate::contracts::IOrderBook;
use crate::grpc_server::QuoteGrpcService;
use crate::models::QuoteRequest;
use crate::order_store::{OrderStore, TrackedOrder};
use crate::transaction_builder::{
    EvmTransactionBuilder, EvmTransactionResult, OpenOrderInput, SvmTransactionBuilder,
    TransactionBuilderError, TransactionResult,
};

#[derive(Clone)]
pub struct ApiState {
    pub grpc_service: QuoteGrpcService,
    pub order_store: Arc<OrderStore>,
    pub chains: Vec<ChainConfig>,
    pub assets: Vec<Asset>,
    pub logger: Logger,
}

#[derive(Debug, Deserialize)]
pub struct OrdersQuery {
    pub sender: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrdersResponse {
    pub orders: Vec<TrackedOrder>,
    pub count: usize,
}

/// Full order details from the contract
#[derive(Debug, Serialize)]
pub struct OrderDetails {
    pub order_id: String,
    pub status: String,
    pub version: u16,
    pub sender: String,
    pub nonce: u64,
    pub origin_chain_id: u32,
    pub dest_chain_id: u32,
    pub fill_deadline: u32,
    pub cancel_requested_at: u32,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub amount_out: String,
    pub recipient: String,
    pub solver: String,
}

#[derive(Debug, Serialize)]
pub struct OrderDetailResponse {
    pub order: Option<OrderDetails>,
    pub error: Option<String>,
}

pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/quote", post(handle_quote_request))
        .route("/orders", get(handle_orders_request))
        .route("/orders/:order_id", get(handle_order_detail_request))
        .route("/assets", get(handle_assets_request))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(state))
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn handle_assets_request(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    (StatusCode::OK, Json(state.assets.clone()))
}

async fn handle_quote_request(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<QuoteRequest>,
) -> impl IntoResponse {
    // Get quotes from solvers
    let mut quotes = state.grpc_service.request_quotes(request.clone()).await;

    // If sender_address is provided, build transactions for each non-rejected quote
    if let Some(ref sender_address) = request.sender_address {
        // Find the input chain config
        let input_chain = state
            .chains
            .iter()
            .find(|c| c.chain_id == request.input_chain_id);

        if let Some(chain) = input_chain {
            let fill_deadline = chrono::Utc::now().timestamp() as u64 + 3600; // 1h from now
            let recipient = request.recipient.as_ref().unwrap_or(sender_address);

            for quote in quotes.iter_mut() {
                if quote.rejected {
                    continue;
                }

                // Parse recipient address
                let recipient_bytes = match parse_address_to_bytes32(recipient) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        warn!(state.logger, "Failed to parse recipient address"; "error" => %e);
                        continue;
                    }
                };

                // Parse solver address
                let solver_bytes = if quote.solver_address.is_empty() {
                    [0u8; 32] // No solver restriction
                } else {
                    match parse_address_to_bytes32(&quote.solver_address) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            warn!(state.logger, "Failed to parse solver address"; "error" => %e);
                            [0u8; 32]
                        }
                    }
                };

                let input = OpenOrderInput {
                    sender_address: sender_address.clone(),
                    token_in: request.input_token.clone(),
                    token_out: request.output_token.clone(),
                    amount_in: request.amount_in,
                    amount_out: quote.output_amount as u128,
                    recipient: recipient_bytes,
                    solver: solver_bytes,
                    dest_chain_id: request.output_chain_id,
                    fill_deadline,
                };

                match chain.chain_type {
                    ChainType::Evm => match build_evm_transaction(chain, &input).await {
                        Ok(result) => {
                            quote.evm_transaction = Some(result.transaction);
                            quote.approval_transaction = result.approval_transaction;
                            quote.order_id = Some(result.order_id);
                            quote.nonce = Some(result.nonce);
                            quote.orderbook_address = Some(chain.order_book_address.clone());
                        }
                        Err(e) => {
                            warn!(state.logger, "Failed to build EVM transaction"; "error" => %e);
                        }
                    },
                    ChainType::Svm => match build_svm_transaction(chain, &input).await {
                        Ok(result) => {
                            quote.svm_transaction = Some(result.transaction);
                            quote.order_id = Some(result.order_id);
                            quote.nonce = Some(result.nonce);
                            quote.orderbook_address = Some(chain.order_book_address.clone());
                        }
                        Err(e) => {
                            warn!(state.logger, "Failed to build SVM transaction"; "error" => %e);
                        }
                    },
                }
            }
        }
    }

    (StatusCode::OK, Json(quotes))
}

/// Build EVM transaction calldata
async fn build_evm_transaction(
    chain: &ChainConfig,
    input: &OpenOrderInput,
) -> Result<EvmTransactionResult, TransactionBuilderError> {
    let builder = EvmTransactionBuilder::new(
        chain.rpc_url.clone(),
        chain.order_book_address.clone(),
        chain.chain_id,
    )?;
    builder.build_open_order_calldata(input).await
}

/// Build SVM transaction
async fn build_svm_transaction(
    chain: &ChainConfig,
    input: &OpenOrderInput,
) -> Result<TransactionResult, TransactionBuilderError> {
    let builder = SvmTransactionBuilder::new(
        chain.rpc_url.clone(),
        Some(chain.order_book_address.clone()),
        chain.chain_id,
    )?;
    builder.build_open_order_transaction(input).await
}

/// Parse an address string (hex or base58) to bytes32
fn parse_address_to_bytes32(address: &str) -> Result<[u8; 32], String> {
    // Try hex (with or without 0x prefix)
    let hex_str = address.strip_prefix("0x").unwrap_or(address);
    if hex_str.len() == 40 {
        // EVM address - left-pad with zeros
        let mut bytes = [0u8; 32];
        let addr_bytes = hex::decode(hex_str).map_err(|e| e.to_string())?;
        bytes[12..].copy_from_slice(&addr_bytes);
        return Ok(bytes);
    }
    if hex_str.len() == 64 {
        // Full bytes32
        let bytes: [u8; 32] = hex::decode(hex_str)
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|_| "Invalid length")?;
        return Ok(bytes);
    }

    // Try base58 (Solana pubkey)
    let decoded = bs58::decode(address)
        .into_vec()
        .map_err(|e| e.to_string())?;
    if decoded.len() == 32 {
        let bytes: [u8; 32] = decoded.try_into().map_err(|_| "Invalid length")?;
        return Ok(bytes);
    }

    Err(format!("Cannot parse address: {}", address))
}

async fn handle_orders_request(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<OrdersQuery>,
) -> impl IntoResponse {
    let orders = if let Some(sender) = query.sender {
        state.order_store.get_orders_by_sender(&sender).await
    } else {
        state.order_store.get_all_orders().await
    };

    let count = orders.len();

    (StatusCode::OK, Json(OrdersResponse { orders, count }))
}

async fn handle_order_detail_request(
    State(state): State<Arc<ApiState>>,
    Path(order_id): Path<String>,
) -> impl IntoResponse {
    let tracked_order = state.order_store.get_order(&order_id).await;

    let chain = if let Some(ref order) = tracked_order {
        // Find the chain config for this order's origin chain
        state
            .chains
            .iter()
            .find(|c| c.chain_id == order.origin_chain_id)
    } else {
        // If we don't have the order, try each chain
        None
    };

    // Parse order_id as bytes32
    let order_id_bytes: FixedBytes<32> = match FixedBytes::from_str(&order_id) {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(OrderDetailResponse {
                    order: None,
                    error: Some("Invalid order_id format".to_string()),
                }),
            )
        }
    };

    // If we know the chain, query just that chain
    if let Some(chain) = chain {
        match fetch_order_from_chain(chain, order_id_bytes).await {
            Ok(Some(details)) => {
                return (
                    StatusCode::OK,
                    Json(OrderDetailResponse {
                        order: Some(details),
                        error: None,
                    }),
                )
            }
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(OrderDetailResponse {
                        order: None,
                        error: Some("Order not found".to_string()),
                    }),
                )
            }
            Err(e) => {
                error!(state.logger, "Error fetching order"; "error" => %e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(OrderDetailResponse {
                        order: None,
                        error: Some(format!("Error fetching order: {}", e)),
                    }),
                );
            }
        }
    }

    // Try all chains if we don't know which one
    for chain in &state.chains {
        match fetch_order_from_chain(chain, order_id_bytes).await {
            Ok(Some(details)) => {
                return (
                    StatusCode::OK,
                    Json(OrderDetailResponse {
                        order: Some(details),
                        error: None,
                    }),
                )
            }
            Ok(None) => continue,
            Err(_) => continue,
        }
    }

    (
        StatusCode::NOT_FOUND,
        Json(OrderDetailResponse {
            order: None,
            error: Some("Order not found on any chain".to_string()),
        }),
    )
}

async fn fetch_order_from_chain(
    chain: &ChainConfig,
    order_id: FixedBytes<32>,
) -> Result<Option<OrderDetails>, Box<dyn std::error::Error + Send + Sync>> {
    match chain.chain_type {
        ChainType::Evm => fetch_order_from_evm_chain(chain, order_id).await,
        ChainType::Svm => fetch_order_from_svm_chain(chain, order_id.as_slice()).await,
    }
}

async fn fetch_order_from_evm_chain(
    chain: &ChainConfig,
    order_id: FixedBytes<32>,
) -> Result<Option<OrderDetails>, Box<dyn std::error::Error + Send + Sync>> {
    let rpc_url = chain.rpc_url.parse()?;
    let provider = ProviderBuilder::new().connect_http(rpc_url);
    let contract_address = Address::from_str(&chain.order_book_address)?;

    let contract = IOrderBook::new(contract_address, &provider);
    let result = contract.getOrder(order_id).call().await?;

    // Check if order exists (status 0 = DoesNotExist)
    let status_num: u8 = result.status.into();
    if status_num == 0 {
        return Ok(None);
    }

    let status = match status_num {
        1 => "created",
        2 => "cancel_requested",
        3 => "completed",
        _ => "unknown",
    };

    Ok(Some(OrderDetails {
        order_id: format!("{:x}", order_id),
        status: status.to_string(),
        version: result.version,
        sender: format!("{:?}", result.sender),
        nonce: result.nonce,
        origin_chain_id: chain.chain_id,
        dest_chain_id: result.destChainId,
        fill_deadline: result.fillDeadline,
        cancel_requested_at: result.cancelRequestedAt,
        token_in: format!("{:?}", result.tokenIn),
        token_out: format!("{:x}", result.tokenOut),
        amount_in: result.amountIn.to_string(),
        amount_out: result.amountOut.to_string(),
        recipient: format!("{:x}", result.recipient),
        solver: format!("{:x}", result.solver),
    }))
}

/// Seed prefix for order PDAs (must match the SVM program)
const ORDER_SEED_PREFIX: &[u8] = b"order";

/// SVM OrderStatus enum (must match the SVM program)
#[derive(BorshDeserialize, Debug, Clone, PartialEq)]
#[repr(u8)]
enum SvmOrderStatus {
    DoesNotExist,
    Created,
    CancelRequested,
    Completed,
}

/// SVM OrderType enum (must match the SVM program)
#[derive(BorshDeserialize, Debug, Clone)]
#[repr(u8)]
enum SvmOrderType {
    Native,
    Foreign,
}

/// SVM NativeOrder struct (must match the SVM program)
#[derive(BorshDeserialize, Debug)]
struct SvmNativeOrder {
    pub status: SvmOrderStatus,
    pub version: u16,
    pub sender: Pubkey,
    pub nonce: u64,
    pub dest_chain_id: u32,
    pub fill_deadline: u64,
    pub cancel_requested_at: u64,
    pub token_in: Pubkey,
    pub token_out: [u8; 32],
    pub amount_in: u128,
    pub amount_out: u128,
    pub recipient: [u8; 32],
    pub solver: [u8; 32],
    pub amount_in_released: u128,
    pub amount_out_filled: u128,
}

/// SVM Order wrapper struct (must match the SVM program)
#[derive(BorshDeserialize, Debug)]
struct SvmOrder {
    pub order_type: SvmOrderType,
    pub bump: u8,
    pub data: SvmNativeOrder,
}

async fn fetch_order_from_svm_chain(
    chain: &ChainConfig,
    order_id: &[u8],
) -> Result<Option<OrderDetails>, Box<dyn std::error::Error + Send + Sync>> {
    let client = RpcClient::new(chain.rpc_url.clone());
    let program_id = Pubkey::from_str(&chain.order_book_address)?;

    // Derive order PDA
    let (order_pda, _) =
        Pubkey::find_program_address(&[ORDER_SEED_PREFIX, order_id], &program_id);

    // Fetch account data
    let account_data = match client.get_account_data(&order_pda).await {
        Ok(data) => data,
        Err(_) => return Ok(None), // Account doesn't exist
    };

    // Skip 8-byte Anchor discriminator and deserialize
    if account_data.len() < 8 {
        return Ok(None);
    }
    let mut slice = &account_data[8..];
    let order: SvmOrder = SvmOrder::deserialize(&mut slice)?;

    // Check if order exists
    if order.data.status == SvmOrderStatus::DoesNotExist {
        return Ok(None);
    }

    let status = match order.data.status {
        SvmOrderStatus::DoesNotExist => "does_not_exist",
        SvmOrderStatus::Created => "created",
        SvmOrderStatus::CancelRequested => "cancel_requested",
        SvmOrderStatus::Completed => "completed",
    };

    Ok(Some(OrderDetails {
        order_id: hex::encode(order_id),
        status: status.to_string(),
        version: order.data.version,
        sender: order.data.sender.to_string(),
        nonce: order.data.nonce,
        origin_chain_id: chain.chain_id,
        dest_chain_id: order.data.dest_chain_id,
        fill_deadline: order.data.fill_deadline as u32,
        cancel_requested_at: order.data.cancel_requested_at as u32,
        token_in: order.data.token_in.to_string(),
        token_out: hex::encode(order.data.token_out),
        amount_in: order.data.amount_in.to_string(),
        amount_out: order.data.amount_out.to_string(),
        recipient: hex::encode(order.data.recipient),
        solver: hex::encode(order.data.solver),
    }))
}
