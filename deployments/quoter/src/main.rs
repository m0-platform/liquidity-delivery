mod api_server;
mod config;
mod grpc_server;
mod models;
mod transaction_builder;

use std::env;

use api_server::{create_router, ApiState};
use config::QuoterConfig;
use grpc_server::QuoteGrpcService;
use slog::{info, Drain, Logger};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let environment = env::var("QUOTER_ENV").unwrap_or_else(|_| "development".to_string());

    let drain = {
        let decorator = slog_term::PlainDecorator::new(std::io::stdout());
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

    // Load config for transaction builder (optional - gracefully handle missing config)
    let config_path = env::var("QUOTER_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());
    let config = QuoterConfig::from_file(&config_path).ok();

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
    let api_state = ApiState {
        grpc_service: grpc_service.clone(),
        chains,
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
