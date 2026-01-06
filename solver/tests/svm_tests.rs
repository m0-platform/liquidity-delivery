mod common;
mod fixtures;

use anchor_client::solana_sdk::pubkey::Pubkey;
use fixtures::SvmChainTestSuite;
use serde_json::json;
use test_context::test_context;

#[test_context(SvmChainTestSuite)]
#[tokio::test]
async fn svm_orderbook_initailized(ctx: &SvmChainTestSuite) {
    let program_id = Pubkey::try_from("MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK")
        .expect("Invalid program ID");

    let (global_pda, _) = Pubkey::find_program_address(&[b"global"], &program_id);

    // Make RPC call to get account info
    let client = reqwest::Client::new();
    let response = client
        .post(ctx.surfpool_endpoint())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                global_pda.to_string(),
                { "encoding": "base64" }
            ]
        }))
        .send()
        .await
        .expect("Failed to send RPC request");

    let json_response: serde_json::Value =
        response.json().await.expect("Failed to parse RPC response");

    // Check that the account is not null
    assert!(
        !json_response["result"]["value"].is_null(),
        "OrderBook global account should be initialized but was null"
    );
}

#[test_context(SvmChainTestSuite)]
#[tokio::test]
async fn svm_orderbook_initailized_b(ctx: &SvmChainTestSuite) {
    let program_id = Pubkey::try_from("MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK")
        .expect("Invalid program ID");

    let (global_pda, _) = Pubkey::find_program_address(&[b"global"], &program_id);

    // Make RPC call to get account info
    let client = reqwest::Client::new();
    let response = client
        .post(ctx.surfpool_endpoint())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                global_pda.to_string(),
                { "encoding": "base64" }
            ]
        }))
        .send()
        .await
        .expect("Failed to send RPC request");

    let json_response: serde_json::Value =
        response.json().await.expect("Failed to parse RPC response");

    // Check that the account is not null
    assert!(
        !json_response["result"]["value"].is_null(),
        "OrderBook global account should be initialized but was null"
    );
}

#[test_context(SvmChainTestSuite)]
#[tokio::test]
async fn test_order_rejected(ctx: &SvmChainTestSuite) {
    let chain = &ctx.chains[0];

    ctx.create_order(
        chain,
        chain.tokens[0].address,
        // Unsupported token
        alloy::primitives::Address::new([0u8; 20]),
        ctx.chains[0].chain_id,
        1000000,
        1000000,
    )
    .await;

    ctx.contains_log("OrderRejected").await;
    ctx.contains_log("Asset not supported").await;
}
