mod common;
mod fixtures;

use anchor_client::solana_sdk::pubkey::Pubkey;
use fixtures::SvmChainTestSuite;
use solana_client::nonblocking::rpc_client::RpcClient;
use test_context::test_context;

#[test_context(SvmChainTestSuite)]
#[tokio::test]
async fn svm_orderbook_initailized(ctx: &SvmChainTestSuite) {
    let program_id = Pubkey::try_from("MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK")
        .expect("Invalid program ID");

    let (global_pda, _) = Pubkey::find_program_address(&[b"global"], &program_id);
    let client = RpcClient::new(ctx.surfpool_endpoint());

    let account = client
        .get_account(&global_pda)
        .await
        .expect("Account not found");

    assert!(
        account.data.len() > 0,
        "OrderBook global account should be initialized but was null"
    );
}

#[test_context(SvmChainTestSuite)]
#[tokio::test]
async fn test_order_from_svm(ctx: &SvmChainTestSuite) {
    let order_id = ctx
        .create_svm_order(
            &ctx.svm_mint.unwrap(),
            ctx.chains[0].tokens[0].address.clone(),
            ctx.chains[0].chain_id,
            1000000,
            1000000,
        )
        .await;

    ctx.contains_order_lifecycle(
        &order_id,
        &[
            "OrderCreated",
            "HoldSuccessful",
            "RequestFillOrder",
            "FillOrderSuccessful",
        ],
    )
    .await;
}
