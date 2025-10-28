use solver::config::{Config, Environment};
use std::error::Error;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = dotenvy::dotenv();
    let config = Config::from_env()?;

    if config.environment == Environment::Production {
        // JSON format for production
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().json())
            .with(config.log_level)
            .init();
    } else {
        // Human-readable format for development
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(config.log_level)
            .init();
    }

    let shutdown_tx = solver::run_solver(config).await?;

    // Wait for SIGINT (Ctrl+C)
    tokio::signal::ctrl_c().await?;
    tracing::info!("Received shutdown signal");
    let _ = shutdown_tx.send(());

    // Wait for components to shutdown gracefully
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    Ok(())
}
