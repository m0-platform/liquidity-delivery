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
async fn test_order_from_svm(ctx: &SvmChainTestSuite) {
    ctx.create_svm_order(
        &ctx.svm_mint.unwrap(),
        ctx.chains[0].tokens[0].address.clone(),
        ctx.chains[0].chain_id,
        1000000,
        1000000,
    )
    .await;

    ctx.contains_order_lifecycle(
        "77bf9f8455c1d9dcd84b9f15a8f3ddd6cd3788a7df3aea845525be85a87dcc62",
        &[
            "OrderCreated",
            "HoldSuccessful",
            "RequestFillOrder",
            "FillOrderSuccessful",
        ],
    )
    .await;
}
