mod common;
mod fixtures;

use fixtures::TestSuite;
use test_context::test_context;

#[test_context(TestSuite)]
#[tokio::test]
async fn test_inventory_manager_loads_balances(ctx: &TestSuite) {
    ctx.contains_log(r"USDC \(Ethereum\): 100");
    ctx.contains_log(r"USDC \(Base\): 100");
    ctx.contains_log(r"USDT \(Ethereum\): 100");
    ctx.contains_log(r"USDT \(Base\): 100");
    ctx.contains_log(r"USDS \(Ethereum\): 100");
    ctx.contains_log(r"USDS \(Base\): 100");
    ctx.contains_log(r"ETH \(Ethereum\): 0.99");
    ctx.contains_log(r"ETH \(Base\): 0.99");
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

    ctx.contains_log("OrderRejected");
    ctx.contains_log("Asset not supported");
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
        "682d2d2fd1e49b926bd2dcd2eabc9285bc2afd9692e71cb8c02aebd916112dd8",
        &[
            "OrderCreated",
            "HoldSuccessful",
            "RequestFillOrder",
            "RequestFillOrder",
            "FillOrderSuccessful",
        ],
    );
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
        "b5a03f77fb3f31d440d42c19eb2fd109774b3f07169ebb4faac81f72e521fe00",
        &[
            "OrderCreated",
            "HoldSuccessful",
            "RequestFillOrder",
            "RequestFillOrder",
            "FillOrderSuccessful",
        ],
    );
}
