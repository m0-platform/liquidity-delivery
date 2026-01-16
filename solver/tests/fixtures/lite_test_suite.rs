use std::ops::Deref;
use test_context::AsyncTestContext;

use crate::common::{MockQuoterHandle, MockQuoterServer};
use crate::fixtures::BaseTestSuite;

#[allow(dead_code)]
pub struct LiteTestSuite {
    base: BaseTestSuite,
    pub mock_quoter: MockQuoterHandle,
}

impl Deref for LiteTestSuite {
    type Target = BaseTestSuite;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl LiteTestSuite {
    /// Get a reference to the mock quoter handle for test interactions
    pub fn quoter(&self) -> &MockQuoterHandle {
        &self.mock_quoter
    }
}

impl AsyncTestContext for LiteTestSuite {
    async fn setup() -> LiteTestSuite {
        // Start mock gRPC quoter server first
        let mock_server = MockQuoterServer::new();
        let mock_quoter = mock_server.start().await;

        // Setup base test suite with the mock quoter URL
        let base = BaseTestSuite::setup_with_mock_quoter(mock_quoter.grpc_url()).await;

        LiteTestSuite { base, mock_quoter }
    }

    async fn teardown(self) {
        self.base.base_teardown().await;
    }
}
