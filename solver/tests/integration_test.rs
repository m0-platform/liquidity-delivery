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
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::IOrderBook::OnchainOrderParams;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    OrderBook,
    "../evm/out/OrderBook.sol/OrderBook.json"
);

#[tokio::test]
async fn test_create_order() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(LevelFilter::INFO)
        .init();

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
    sleep(Duration::from_secs(1)).await;

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
    sleep(Duration::from_secs(5)).await;

    // Send shutdown signal
    let _ = shutdown_tx.send(());

    // Wait for graceful shutdown
    sleep(Duration::from_secs(1)).await;

    // If we got here without panicking, the test passed
    tracing::info!("Solver successfully started and shut down");
}
