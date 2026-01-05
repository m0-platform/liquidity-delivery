mod common;
mod fixtures;

use quoter::{QuoteRequest, QuoteResponse};
use test_context::test_context;

use crate::fixtures::LiteTestSuite;

#[test_context(LiteTestSuite)]
#[tokio::test]
async fn test_quote_invalid_asset(ctx: &LiteTestSuite) {
    let request = QuoteRequest {
        input_chain_id: 1,
        output_chain_id: 4294967295,
        input_token: "0x437cc33344a0B27A429f795ff6B469C72698B291".to_string(),
        output_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
        amount_in: 100000,
    };

    let responses: Vec<QuoteResponse> = reqwest::Client::new()
        .post(ctx.quote_endpoint())
        .json(&request)
        .send()
        .await
        .expect("quote call failed")
        .json()
        .await
        .expect("failed to parse quote response");

    assert_eq!(responses.len(), 1, "Expected 1 quote response");

    let quote = &responses[0];
    assert!(quote.rejected && !quote.reject_reason.is_empty());
    assert_eq!(
        quote.reject_reason, "output_token: Asset not supported",
        "Reject reason was {}",
        quote.reject_reason
    );
}

#[test_context(LiteTestSuite)]
#[tokio::test]
async fn test_quote_success(ctx: &LiteTestSuite) {
    let token_addr = ctx.chains[0].tokens[0].address.to_string();

    let request = QuoteRequest {
        input_chain_id: 1,
        output_chain_id: 8453,
        input_token: token_addr.clone(),
        output_token: token_addr,
        amount_in: 100000,
    };

    let responses: Vec<QuoteResponse> = reqwest::Client::new()
        .post(ctx.quote_endpoint())
        .json(&request)
        .send()
        .await
        .expect("quote call failed")
        .json()
        .await
        .expect("failed to parse quote response");

    assert_eq!(responses.len(), 1, "Expected 1 quote response");

    let quote = &responses[0];
    assert!(
        !quote.rejected,
        "Quote was unexpectedly rejected: {:?}",
        quote
    );
    assert_eq!(
        quote.output_amount, 100000,
        "Unexpected output amount: {}",
        quote.output_amount
    );
}
