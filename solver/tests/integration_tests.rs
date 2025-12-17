mod common;
mod fixtures;

use fixtures::TestSuite;
use test_context::test_context;

#[test_context(TestSuite)]
#[tokio::test]
async fn test_inventory_manager_loads_balances(ctx: &TestSuite) {
    for log in [
        r"USDC \(Ethereum\): 999999999999900",
        r"USDC \(Base\): 999999999999900",
        r"USDT \(Ethereum\): 999999999999900",
        r"USDT \(Base\): 999999999999900",
        r"USDS \(Ethereum\): 999999999999900",
        r"USDS \(Base\): 999999999999900",
        r"ETH \(Ethereum\): 0.99",
        r"ETH \(Base\): 0.99",
    ] {
        ctx.contains_log(log).await;
    }
}

#[test_context(TestSuite)]
#[tokio::test]
async fn test_order_rejected(ctx: &TestSuite) {
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

#[test_context(TestSuite)]
#[tokio::test]
async fn test_order_processed_chain_a(ctx: &TestSuite) {
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

#[test_context(TestSuite)]
#[tokio::test]
async fn test_order_processed_chain_b(ctx: &TestSuite) {
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
