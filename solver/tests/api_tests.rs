mod common;
mod fixtures;

use fixtures::LiteTestSuite;
use serde_json::Value;
use solver::api::{QuoteRequest, QuoteResponse};
use test_context::test_context;

const HEALTH_ENDPOINT: &str = "http://localhost:3000/health";
const QUOTE_ENDPOINT: &str = "http://localhost:3000/quote";

#[test_context(LiteTestSuite)]
#[tokio::test]
async fn test_health(_ctx: &LiteTestSuite) {
    let resp = reqwest::get(HEALTH_ENDPOINT)
        .await
        .expect("failed to call health endpoint");
    assert!(resp.status().is_success(), "health endpoint returned error");

    let body: Value = resp.json().await.expect("failed to parse json");
    let message = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
    assert_eq!(message, "Healthy");
}

#[test_context(LiteTestSuite)]
#[tokio::test]
async fn test_quote_invalid_asset(_ctx: &LiteTestSuite) {
    let request = QuoteRequest {
        input_chain_id: 1,
        output_chain_id: 4294967295,
        input_token: "0x437cc33344a0B27A429f795ff6B469C72698B291".to_string(),
        output_token: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
        amount_in: 100000,
    };

    let quote: QuoteResponse = reqwest::Client::new()
        .post(QUOTE_ENDPOINT)
        .json(&request)
        .send()
        .await
        .expect("quote call failed")
        .json()
        .await
        .expect("failed to parse quote response");

    assert!(quote.rejected && quote.reject_reason.is_some());
    assert_eq!(
        quote.clone().reject_reason.unwrap(),
        "output_token: Asset not supported",
        "Reject reason was {}",
        quote.reject_reason.unwrap()
    );
}

#[test_context(LiteTestSuite)]
#[tokio::test]
async fn test_quote_success(_ctx: &LiteTestSuite) {
    let request = QuoteRequest {
        input_chain_id: 1,
        output_chain_id: 8453,
        input_token: "0x437cc33344a0B27A429f795ff6B469C72698B291".to_string(),
        output_token: "0x437cc33344a0B27A429f795ff6B469C72698B291".to_string(),
        amount_in: 100000,
    };

    let quote: QuoteResponse = reqwest::Client::new()
        .post(QUOTE_ENDPOINT)
        .json(&request)
        .send()
        .await
        .expect("quote call failed")
        .json()
        .await
        .expect("failed to parse quote response");

    assert!(
        !quote.rejected,
        "Quote was unexpectedly rejected: {:?}",
        quote
    );
    assert_eq!(quote.fee_bps, 0);
    assert_eq!(
        quote.output_amount, 100000,
        "Unexpected output amount: {}",
        quote.output_amount
    );
}
