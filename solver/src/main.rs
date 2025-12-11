use slog::{info, Drain, Logger};
use solver::common_logger_values;
use solver::config::{Config, Environment};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yaml".to_string());

    let config = Config::from_file(&config_path)?;

    // Create the root logger
    let logger = if config.environment == Environment::Production {
        // JSON format for production
        let drain = slog_json::Json::default(std::io::stdout()).fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        Logger::root(drain, common_logger_values!())
    } else {
        // Human-readable format for development
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        Logger::root(drain, common_logger_values!())
    };

    let shutdown_tx = solver::run_solver(config, logger.clone()).await?;

    // Wait for SIGINT (Ctrl+C)
    tokio::signal::ctrl_c().await?;
    info!(logger, "Received shutdown signal");
    let _ = shutdown_tx.send(());

    // Wait for components to shutdown gracefully
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    Ok(())
}
