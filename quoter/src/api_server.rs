use alloy::primitives::{Address, FixedBytes};
use alloy::providers::ProviderBuilder;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use slog::{error, info, Logger};
use std::str::FromStr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::config::{Asset, ChainConfig};
use crate::contracts::IOrderBook;
use crate::grpc_server::QuoteGrpcService;
use crate::models::QuoteRequest;
use crate::order_store::{OrderStore, TrackedOrder};

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
        .route("/orders/{order_id}", get(handle_order_detail_request))
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
    (
        StatusCode::OK,
        Json(state.grpc_service.request_quotes(request).await),
    )
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
    let order_id_bytes: FixedBytes<32> = match FixedBytes::from_str(&format!("0x{}", order_id)) {
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
