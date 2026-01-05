mod api_server;
mod grpc_server;
mod models;

use api_server::{create_router, ApiState};
use grpc_server::QuoteGrpcService;
use slog::{info, Drain, Logger};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let environment = std::env::var("QUOTER_ENV").unwrap_or_else(|_| "development".to_string());

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

    let logger = Logger::root(
        drain,
        slog::o!(
            "timestamp" => slog::FnValue(|_| {
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            }),
            "environment" => environment.clone()
        ),
    );

    let quote_timeout_ms: u64 = std::env::var("QUOTE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);

    info!(logger, "Starting quoter"; "environment" => &environment, "quote_timeout_ms" => quote_timeout_ms);

    let grpc_service = QuoteGrpcService::new(quote_timeout_ms, logger.clone());

    let grpc_addr = "[::1]:50051".parse()?;
    let api_addr = "0.0.0.0:3000";

    // Spawn gRPC server
    let grpc_service_clone = grpc_service.clone();
    let grpc_server = tokio::spawn(async move {
        Server::builder()
            .add_service(grpc_service_clone.get_server())
            .serve(grpc_addr)
            .await
    });

    // Spawn HTTP API server
    let api_state = ApiState {
        grpc_service: grpc_service.clone(),
        logger: logger.clone(),
    };

    let app = create_router(api_state);
    let listener = tokio::net::TcpListener::bind(api_addr).await?;
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
