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
        chain.tokens[0].address,
        // Unsupported token
        alloy::primitives::Address::new([0u8; 20]),
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
        chain_a.tokens[0].address,
        chain_b.tokens[1].address,
        chain_b.chain_id,
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

#[test_context(EvmChainTestSuite)]
#[tokio::test]
async fn test_order_processed_chain_b(ctx: &EvmChainTestSuite) {
    let (chain_a, chain_b) = (&ctx.chains[1], &ctx.chains[0]);

    ctx.create_order(
        chain_a,
        chain_a.tokens[0].address,
        chain_b.tokens[1].address,
        chain_b.chain_id,
        500000,
        500000,
    )
    .await;

    ctx.contains_order_lifecycle(
        "191fe545a21e074a407c6a8c5b34bfbc3925ccd67926384756c60fce8cbad58b",
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
        chain_a.tokens[0].address,
        chain_b.tokens[2].address,
        chain_b.chain_id,
        500000,
        5000000,
    )
    .await;

    ctx.contains_order_lifecycle(
        "ce8959a0ac771c894491f414ae46149f9a6960e7aa43ac74740ef2a3fc4827ed",
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
        chain_a.tokens[0].address,
        chain_b.tokens[2].address,
        chain_b.chain_id,
        50000000,
        50000000,
    )
    .await;

    ctx.contains_order_lifecycle(
        "c2880ef255c9dc5f304bb77d13d8b61d73be02b7fb4a428396ecb4182dedea06",
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
        chain_a.tokens[2].address,
        chain_b.tokens[0].address,
        chain_b.chain_id,
        // max clip size is $100
        150_000_000,
        150_000_000,
    )
    .await;

    // Fill order in two clips
    ctx.contains_order_lifecycle(
        "c9f50de3522efc0bca73b996ab5916ad68ec65d9be0229d76c00ae31ef48f466",
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
