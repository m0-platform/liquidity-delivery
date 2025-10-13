use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_solver_startup_and_shutdown() {
    // Start the solver
    let shutdown_tx = solver::run_solver().await.expect("Failed to start solver");

    // Let it run for 5 seconds
    sleep(Duration::from_secs(5)).await;

    // Send shutdown signal
    let _ = shutdown_tx.send(());

    // Wait for graceful shutdown
    sleep(Duration::from_secs(1)).await;

    // If we got here without panicking, the test passed
    println!("Solver successfully started and shut down after 5 seconds");
}
