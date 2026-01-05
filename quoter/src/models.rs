use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub input_token: String,
    pub input_chain_id: u32,
    pub output_token: String,
    pub output_chain_id: u32,
    pub amount_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteResponse {
    pub quote_id: String,
    pub fee_bps: u32,
    pub output_amount: u64,
    pub est_fill_time_seconds: u64,
    pub expires_at: String,
    pub rejected: bool,
    pub reject_reason: String,
    pub solver_address: String,
    pub requires_exclusivity: bool,
}
