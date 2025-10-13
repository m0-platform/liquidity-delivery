use alloy::{
    hex,
    primitives::{aliases::U40, FixedBytes},
    providers::ProviderBuilder,
    sol,
};
use std::{env, time::Duration};
use tokio::time::sleep;

use crate::IOrderBook::OnchainOrderParams;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    OrderBook,
    "../evm/out/OrderBook.sol/OrderBook.json"
);

#[tokio::test]
async fn test_create_order() {
    let provider = ProviderBuilder::new().connect_anvil_with_wallet();

    let contract = OrderBook::deploy(&provider)
        .await
        .expect("Failed to deploy contract");

    println!("Deployed contract at address: {}", contract.address());

    // Override values in .env for testing if necessary
    env::set_var("CHAIN_11155111_RPC_URL", "http://localhost:8545");

    // Start the solver
    let shutdown_tx = solver::run_solver().await.expect("Failed to start solver");

    let builder = contract.openOrder(OnchainOrderParams {
        tokenIn: hex!("0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238").into(),
        destChainId: 421614,
        tokenOut: FixedBytes::from([0u8; 32]),
        amountIn: 1000000,
        amountOut: 1000000,
        recipient: FixedBytes::from([0u8; 32]),
        fillDeadline: U40::MAX,
        solver: FixedBytes::from([0u8; 32]),
    });

    builder
        .send()
        .await
        .expect("Failed to send openOrder transaction")
        .watch()
        .await
        .expect("Failed to confirm transaction");

    // Let the solver run and pick up the order
    sleep(Duration::from_secs(5)).await;

    // Send shutdown signal
    let _ = shutdown_tx.send(());

    // Wait for graceful shutdown
    sleep(Duration::from_secs(1)).await;

    // If we got here without panicking, the test passed
    println!("Solver successfully started and shut down");
}
