use std::ops::Deref;
use test_context::AsyncTestContext;

use crate::fixtures::BaseTestSuite;

#[allow(dead_code)]
pub struct LiteTestSuite(BaseTestSuite);

impl Deref for LiteTestSuite {
    type Target = BaseTestSuite;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsyncTestContext for LiteTestSuite {
    async fn setup() -> LiteTestSuite {
        let base = BaseTestSuite::setup_with_chains(vec![], true).await;
        LiteTestSuite(base)
    }

    async fn teardown(self) {
        self.0.base_teardown().await;
    }
}
