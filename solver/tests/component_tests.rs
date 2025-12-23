mod common;
mod fixtures;

use fixtures::EvmChainTestSuite;
use test_context::test_context;

#[test_context(EvmChainTestSuite)]
#[tokio::test]
async fn test_inventory_manager_loads_balances(ctx: &EvmChainTestSuite) {
    for log in [
        r"USDC \(Ethereum\): 999999999999900",
        r"USDC \(Base\): 999999999999900",
        r"USDT \(Ethereum\): 999999999999900",
        r"USDT \(Base\): 999999999999900",
        r"USDS \(Ethereum\): 10",
        r"USDS \(Base\): 10",
        r"ETH \(Ethereum\): 0.99",
        r"ETH \(Base\): 0.99",
    ] {
        ctx.contains_log(log).await;
    }
}
