use async_trait::async_trait;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::Serialize;
use slog::{info, warn};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use m0_liquidity_sdk::types::Asset;

use crate::{
    error::Result,
    events::{EventHandler, EventProcessor, SolverEvent},
    stores::{BalanceStore, Order, OrderStore, TransactionRecord},
    utils::format_address,
};

use super::ComponentParams;

/// Order summary for API response
#[derive(Debug, Serialize)]
pub struct OrderSummary {
    pub order_id: String,
    pub status: String,
    pub version: u16,
    pub nonce: u64,
    pub origin_chain_id: u32,
    pub dest_chain_id: u32,
    pub fill_deadline: u64,
    pub sender: String,
    pub recipient: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub amount_out: String,
    pub filled_amount: String,
    pub solver: String,
    pub created_at: u64,
    pub transaction_history: Vec<TransactionRecord>,
}

impl OrderSummary {
    fn from_order(order: &Order) -> Self {
        Self {
            order_id: order.id.clone(),
            status: order.state.to_string(),
            version: order.data.version,
            nonce: order.data.nonce,
            origin_chain_id: order.data.origin_chain_id,
            dest_chain_id: order.data.dest_chain_id,
            fill_deadline: order.data.fill_deadline,
            sender: format_address(&order.data.sender),
            recipient: format_address(&order.data.recipient),
            token_in: format_address(&order.data.token_in),
            token_out: format_address(&order.data.token_out),
            amount_in: order.data.amount_in.to_string(),
            amount_out: order.data.amount_out.to_string(),
            filled_amount: order.filled_amount.to_string(),
            solver: format_address(&order.data.solver),
            created_at: order.created_at,
            transaction_history: order.transaction_history.clone(),
        }
    }
}

/// Response for GET /orders
#[derive(Debug, Serialize)]
pub struct OrdersResponse {
    pub orders: Vec<OrderSummary>,
    pub count: usize,
}

/// Balance summary for API response
#[derive(Debug, Serialize)]
pub struct BalanceSummary {
    pub chain: String,
    pub address: String,
    pub symbol: String,
    pub decimals: i64,
    pub balance: String,
}

impl BalanceSummary {
    fn from_asset(asset: &Asset, balance: u128) -> Self {
        Self {
            chain: format!("{:?}", asset.chain),
            address: asset.address.clone(),
            symbol: asset.symbol.clone(),
            decimals: asset.decimals,
            balance: balance.to_string(),
        }
    }
}

/// Response for GET /balances
#[derive(Debug, Serialize)]
pub struct BalancesResponse {
    pub balances: Vec<BalanceSummary>,
    pub count: usize,
}

/// Shared state for API routes
#[derive(Clone)]
struct AppState {
    order_store: Arc<OrderStore>,
    balance_store: Arc<BalanceStore>,
}

/// API Server component
pub struct ApiServer {
    port: Option<u16>,
    logger: slog::Logger,
    order_store: Arc<OrderStore>,
    balance_store: Arc<BalanceStore>,
}

impl ApiServer {
    pub fn new(params: &ComponentParams) -> Self {
        let logger = params.logger.new(slog::o!("component" => "ApiServer"));

        Self {
            port: params.config.http_port,
            logger,
            order_store: Arc::new(OrderStore::new()),
            balance_store: Arc::new(BalanceStore::new()),
        }
    }

    fn create_router(&self) -> Router {
        let state = AppState {
            order_store: self.order_store.clone(),
            balance_store: self.balance_store.clone(),
        };

        Router::new()
            .route("/health", get(health_check))
            .route("/orders", get(handle_orders_request))
            .route("/balances", get(handle_balances_request))
            .layer(CorsLayer::permissive())
            .with_state(state)
    }
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn handle_orders_request(State(state): State<AppState>) -> impl IntoResponse {
    let orders = state.order_store.get_all_orders().await;
    let count = orders.len();

    let mut order_list: Vec<OrderSummary> = orders.iter().map(OrderSummary::from_order).collect();
    order_list.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    (
        StatusCode::OK,
        Json(OrdersResponse {
            orders: order_list,
            count,
        }),
    )
}

async fn handle_balances_request(State(state): State<AppState>) -> impl IntoResponse {
    let balances = state.balance_store.get_all_balances().await;
    let count = balances.len();

    let balance_list: Vec<BalanceSummary> = balances
        .iter()
        .map(|(asset, balance)| BalanceSummary::from_asset(asset, *balance))
        .collect();

    (
        StatusCode::OK,
        Json(BalancesResponse {
            balances: balance_list,
            count,
        }),
    )
}

#[async_trait]
impl EventHandler for ApiServer {
    async fn initialize(&self) -> Result<()> {
        self.order_store.initialize().await?;
        self.balance_store.initialize().await?;

        let Some(port) = self.port else {
            warn!(self.logger, "HTTP API port not configured, server disabled");
            return Ok(());
        };

        let addr = format!("0.0.0.0:{}", port);
        let router = self.create_router();
        let logger = self.logger.clone();

        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    slog::error!(logger, "Failed to bind HTTP server"; "error" => %e);
                    return;
                }
            };

            info!(logger, "HTTP API server listening"; "addr" => &addr);

            if let Err(e) = axum::serve(listener, router).await {
                slog::error!(logger, "HTTP API server error"; "error" => %e);
            }
        });

        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        // Only process events if API is enabled
        if self.port.is_none() {
            return Ok(Vec::new());
        }

        let _ = self.order_store.handle_event(event.clone()).await;
        let _ = self.balance_store.handle_event(event.clone()).await;

        Ok(Vec::new())
    }

    fn name(&self) -> &'static str {
        "ApiServer"
    }
}
