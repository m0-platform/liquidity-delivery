mod api_server;
mod config;
mod contracts;
mod evm_order_tracker;
mod grpc_server;
mod models;
mod order_store;
mod svm_order_tracker;
mod transaction_builder;

use std::env;
use std::sync::Arc;

use api_server::{create_router, ApiState};
use config::QuoterConfig;
use evm_order_tracker::EvmOrderTracker;
use grpc_server::QuoteGrpcService;
use order_store::OrderStore;
use slog::{error, info, Drain, Logger};
use svm_order_tracker::SvmOrderTracker;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let environment = env::var("QUOTER_ENV").unwrap_or_else(|_| "development".to_string());

    // Create logger
    let drain = if environment == "production" {
        // JSON format for production
        let drain = slog_json::Json::default(std::io::stdout()).fuse();
        slog_async::Async::new(drain).build().fuse()
    } else {
        // Human-readable format for development
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        slog_async::Async::new(drain).build().fuse()
    };

    let quote_timeout_ms: u64 = std::env::var("QUOTE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);

    let logger = Logger::root(
        drain,
        slog::o!(
            "timestamp" => slog::FnValue(|_| {
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            }),
            "environment" => environment.clone(),
            "quote_timeout" => quote_timeout_ms
        ),
    );

    // Load config for order tracking (optional - gracefully handle missing config)
    let config_path = env::var("QUOTER_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());
    let config = QuoterConfig::from_file(&config_path).ok();

    // Initialize order store
    let order_store = Arc::new(OrderStore::new());

    // Start EVM order tracker if config is available
    let _evm_order_tracker = if let Some(ref cfg) = config {
        let tracker = EvmOrderTracker::new(
            order_store.clone(),
            cfg.enabled_chains(),
            logger.clone(),
        );

        if let Err(e) = tracker.start().await {
            error!(logger, "Failed to start EVM order tracker"; "error" => %e);
        } else {
            info!(logger, "EVM order tracker started"; "chains" => cfg.enabled_chains().len());
        }

        Some(tracker)
    } else {
        info!(logger, "No config file found, EVM order tracking disabled"; "config_path" => &config_path);
        None
    };

    // Start SVM order tracker if config is available
    let _svm_order_tracker = if let Some(ref cfg) = config {
        let tracker = SvmOrderTracker::new(
            order_store.clone(),
            cfg.enabled_chains(),
            logger.clone(),
        );

        if let Err(e) = tracker.start().await {
            error!(logger, "Failed to start SVM order tracker"; "error" => %e);
        } else {
            info!(logger, "SVM order tracker started");
        }

        Some(tracker)
    } else {
        None
    };

    let grpc_service = QuoteGrpcService::new(quote_timeout_ms, logger.clone());
    let grpc_host = env::var("GRPC_HOST").unwrap_or_else(|_| "[::1]".to_string());
    let grpc_port = env::var("GRPC_PORT").unwrap_or_else(|_| "50051".to_string());
    let grpc_addr = format!("{}:{}", grpc_host, grpc_port).parse()?;

    // Spawn gRPC server
    let grpc_service_clone = grpc_service.clone();
    let grpc_server = tokio::spawn(async move {
        Server::builder()
            .add_service(grpc_service_clone.get_server())
            .serve(grpc_addr)
            .await
    });

    // Spawn HTTP API server
    let chains = config
        .as_ref()
        .map(|c| c.enabled_chains())
        .unwrap_or_default();
    let assets = config
        .as_ref()
        .map(|c| c.assets.clone())
        .unwrap_or_default();
    let api_state = ApiState {
        grpc_service: grpc_service.clone(),
        order_store: order_store.clone(),
        chains,
        assets,
        logger: logger.clone(),
    };

    let api_addr = format!(
        "0.0.0.0:{}",
        env::var("API_PORT").unwrap_or_else(|_| "3000".to_string())
    );

    let app = create_router(api_state);
    let listener = tokio::net::TcpListener::bind(api_addr.clone()).await?;
    let api_server = tokio::spawn(async move { axum::serve(listener, app).await });

    info!(
        logger,
        "Servers running";
        "grpc_addr" => %grpc_addr,
        "api_addr" => api_addr
    );

    // Wait for both servers
    tokio::select! {
        result = grpc_server => {
            match result {
                Ok(Ok(_)) => info!(logger, "gRPC server stopped"),
                Ok(Err(e)) => slog::error!(logger, "gRPC server error"; "error" => %e),
                Err(e) => slog::error!(logger, "gRPC server task error"; "error" => %e),
            }
        }
        result = api_server => {
            match result {
                Ok(Ok(_)) => info!(logger, "API server stopped"),
                Ok(Err(e)) => slog::error!(logger, "API server error"; "error" => %e),
                Err(e) => slog::error!(logger, "API server task error"; "error" => %e),
            }
        }
    }

    Ok(())
}
