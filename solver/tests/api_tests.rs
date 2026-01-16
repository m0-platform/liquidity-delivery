mod common;
mod fixtures;

use test_context::test_context;

use crate::common::mock_quoter::proto::QuoteRequestProto;
use crate::fixtures::LiteTestSuite;

#[test_context(LiteTestSuite)]
#[tokio::test]
async fn test_quote_invalid_asset(ctx: &LiteTestSuite) {
    // Use valid chain IDs but an unsupported output token
    let request = QuoteRequestProto {
        request_id: "test-invalid-asset-1".to_string(),
        input_chain_id: 1,
        output_chain_id: 8453,
        input_token: "0x437cc33344a0B27A429f795ff6B469C72698B291".to_string(),
        // Use a token address that is not in the supported assets list
        output_token: "0x0000000000000000000000000000000000000001".to_string(),
        amount_in: 100000,
    };

    // Send quote request via gRPC
    ctx.quoter()
        .send_quote_request(request)
        .expect("Failed to send quote request");

    // Wait for response from solver
    let response = ctx
        .quoter()
        .recv_quote_response(5000)
        .await
        .expect("Failed to receive quote response");

    assert_eq!(response.request_id, "test-invalid-asset-1");
    assert!(response.rejected, "Expected quote to be rejected");
    assert!(
        !response.reject_reason.is_empty(),
        "Expected reject reason to be non-empty"
    );
}

#[test_context(LiteTestSuite)]
#[tokio::test]
async fn test_quote_success(ctx: &LiteTestSuite) {
    let request = QuoteRequestProto {
        request_id: "test-success-1".to_string(),
        input_chain_id: 1,
        output_chain_id: 8453,
        input_token: "0x437cc33344a0B27A429f795ff6B469C72698B291".to_string(),
        output_token: "0x437cc33344a0B27A429f795ff6B469C72698B291".to_string(),
        amount_in: 100000,
    };

    // Send quote request via gRPC
    ctx.quoter()
        .send_quote_request(request)
        .expect("Failed to send quote request");

    // Wait for response from solver
    let response = ctx
        .quoter()
        .recv_quote_response(5000)
        .await
        .expect("Failed to receive quote response");

    assert_eq!(response.request_id, "test-success-1");
    assert!(
        !response.rejected,
        "Quote was unexpectedly rejected: {}",
        response.reject_reason
    );
    assert_eq!(
        response.output_amount, 100000,
        "Unexpected output amount: {}",
        response.output_amount
    );
}
