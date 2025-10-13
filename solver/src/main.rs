use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let shutdown_tx = solver::run_solver().await?;

    // Wait for SIGINT (Ctrl+C)
    tokio::signal::ctrl_c().await?;
    tracing::info!("Received shutdown signal");
    let _ = shutdown_tx.send(());

    // Wait for components to shutdown gracefully
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    Ok(())
}
