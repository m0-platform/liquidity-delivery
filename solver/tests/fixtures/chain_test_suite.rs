use std::ops::Deref;
use test_context::AsyncTestContext;

use crate::fixtures::BaseTestSuite;

#[allow(dead_code)]
pub struct EvmChainTestSuite(BaseTestSuite);

impl Deref for EvmChainTestSuite {
    type Target = BaseTestSuite;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsyncTestContext for EvmChainTestSuite {
    async fn setup() -> EvmChainTestSuite {
        let base = BaseTestSuite::setup_with_chains(vec![1, 8453], false).await;
        EvmChainTestSuite(base)
    }

    async fn teardown(self) {
        self.0.base_teardown().await;
    }
}

#[allow(dead_code)]
pub struct SvmChainTestSuite(BaseTestSuite);

impl Deref for SvmChainTestSuite {
    type Target = BaseTestSuite;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsyncTestContext for SvmChainTestSuite {
    async fn setup() -> SvmChainTestSuite {
        let base = BaseTestSuite::setup_with_chains(vec![1, 1399811149], false).await;
        SvmChainTestSuite(base)
    }

    async fn teardown(self) {
        self.0.base_teardown().await;
    }
}
