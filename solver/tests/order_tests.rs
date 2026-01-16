mod common;
mod fixtures;

use fixtures::EvmChainTestSuite;
use test_context::test_context;

#[test_context(EvmChainTestSuite)]
#[tokio::test]
async fn test_order_rejected(ctx: &EvmChainTestSuite) {
    let chain = &ctx.chains[0];

    ctx.create_order(
        chain,
        chain.tokens[0].address.clone(),
        // Unsupported token
        alloy::primitives::Address::new([0u8; 20]).to_string(),
        ctx.chains[1].chain_id,
        1000000,
        1000000,
    )
    .await;

    ctx.contains_log("OrderRejected").await;
    ctx.contains_log("Asset not supported").await;
}

#[test_context(EvmChainTestSuite)]
#[tokio::test]
async fn test_order_processed_chain_a(ctx: &EvmChainTestSuite) {
    let (chain_a, chain_b) = (&ctx.chains[0], &ctx.chains[1]);

    ctx.create_order(
        chain_a,
        chain_a.tokens[0].address.clone(),
        chain_b.tokens[1].address.clone(),
        chain_b.chain_id,
        1000000,
        1000000,
    )
    .await;

    ctx.contains_order_lifecycle(
        "bb2ebc4d21ccb42335c3280f1ae67382f0e01b3338c611cc89261f62fbee864a",
        &[
            "OrderCreated",
            "HoldSuccessful",
            "RequestFillOrder",
            "FillOrderSuccessful",
        ],
    )
    .await;
}

#[test_context(EvmChainTestSuite)]
#[tokio::test]
async fn test_order_processed_chain_b(ctx: &EvmChainTestSuite) {
    let (chain_a, chain_b) = (&ctx.chains[1], &ctx.chains[0]);

    ctx.create_order(
        chain_a,
        chain_a.tokens[0].address.clone(),
        chain_b.tokens[1].address.clone(),
        chain_b.chain_id,
        500000,
        500000,
    )
    .await;

    ctx.contains_order_lifecycle(
        "f3d9bb436a1a1bec3fe812c6ab36a7222c36fe5efd682d5384b3a4cf4503e3b4",
        &[
            "OrderCreated",
            "HoldSuccessful",
            "RequestFillOrder",
            "FillOrderSuccessful",
        ],
    )
    .await;
}

#[test_context(EvmChainTestSuite)]
#[tokio::test]
async fn test_order_invalid_out(ctx: &EvmChainTestSuite) {
    let (chain_a, chain_b) = (&ctx.chains[1], &ctx.chains[0]);

    ctx.create_order(
        chain_a,
        chain_a.tokens[0].address.clone(),
        chain_b.tokens[2].address.clone(),
        chain_b.chain_id,
        500000,
        5000000,
    )
    .await;

    ctx.contains_order_lifecycle(
        "068ef0ce3108384345eb418a6f3d3bbe11128fccba6841bee043c1468687dede",
        &["OrderCreated", "OrderRejected"],
    )
    .await;

    ctx.contains_log("amount_out 5000000 does not cover fee-inclusive amount_out 500000")
        .await;
}

#[test_context(EvmChainTestSuite)]
#[tokio::test]
async fn test_order_insufficient_solver_funds(ctx: &EvmChainTestSuite) {
    let (chain_a, chain_b) = (&ctx.chains[1], &ctx.chains[0]);

    ctx.create_order(
        chain_a,
        chain_a.tokens[0].address.clone(),
        chain_b.tokens[2].address.clone(),
        chain_b.chain_id,
        50000000,
        50000000,
    )
    .await;

    ctx.contains_order_lifecycle(
        "f363ed204ea9f8e92373a07341e7175c1e0ddb2eeab38b2bdf8c083ff92a83e3",
        &[
            "OrderCreated",
            "HoldSuccessful",
            "RequestSwap",
            "RequestFillOrder",
            "FillOrderSuccessful",
        ],
    )
    .await;
}

#[test_context(EvmChainTestSuite)]
#[tokio::test]
async fn test_order_multiple_clips(ctx: &EvmChainTestSuite) {
    let (chain_a, chain_b) = (&ctx.chains[0], &ctx.chains[1]);

    ctx.create_order(
        chain_a,
        chain_a.tokens[2].address.clone(),
        chain_b.tokens[0].address.clone(),
        chain_b.chain_id,
        // max clip size is $100
        150_000_000,
        150_000_000,
    )
    .await;

    // Fill order in two clips
    ctx.contains_order_lifecycle(
        "b789ee6ee2c03f240b34eea05f0a19f0bd18d26133b477df4723c1b274ce06cb",
        &[
            "OrderCreated",
            "HoldSuccessful",
            "RequestFillOrder",
            "FillOrderSuccessful",
            "HoldSuccessful",
            "RequestFillOrder",
            "FillOrderSuccessful",
        ],
    )
    .await;
}
