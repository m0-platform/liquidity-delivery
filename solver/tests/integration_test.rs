use alloy::{
    hex,
    node_bindings::Anvil,
    primitives::{aliases::U40, FixedBytes},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    sol,
};
use solver::Config;
use std::time::Duration;
use tokio::time::sleep;
use tracing_test::traced_test;

use crate::IOrderBook::OnchainOrderParams;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    OrderBook,
    "../evm/out/OrderBook.sol/OrderBook.json"
);

#[tokio::test]
#[traced_test]
async fn test_order_rejected() {
    let anvil = Anvil::new()
        .block_time(1)
        .chain_id(11155111)
        .try_spawn()
        .expect("failed to spawn anvil node");

    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect_http(anvil.endpoint_url());

    let contract = OrderBook::deploy(&provider)
        .await
        .expect("Failed to deploy contract");

    tracing::info!("Deployed contract at address: {}", contract.address());

    // Start the solver with test configuration
    let mut config = Config::default();
    config.chains.push(solver::config::ChainConfig {
        chain_id: 11155111,
        rpc_url: anvil.endpoint_url().to_string(),
        ws_url: anvil.ws_endpoint_url().to_string(),
        order_book_address: contract.address().to_string(),
    });

    let shutdown_tx = solver::run_solver(config)
        .await
        .expect("Failed to start solver");

    // Let the solver boot up
    sleep(Duration::from_millis(10)).await;

    // Random order that will be rejected by the Solver
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

    tracing::info!("Created order");

    // Let the solver run and pick up the order
    sleep(Duration::from_millis(500)).await;

    // Send shutdown signal
    let _ = shutdown_tx.send(());
    sleep(Duration::from_millis(10)).await;

    // Check that the OrderRejected event was created
    assert!(logs_contain("event=\"OrderRejected\" order_id=6dd444764fa0dc3229a76f38314ae608a4e69cef19bce9c636e4661c7f58117e"));
    assert!(logs_contain("reason=Asset not supported"));
}
