use serde::{Deserialize, Serialize};

use crate::grpc_server::proto::QuoteResponseProto;

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

impl From<QuoteResponseProto> for QuoteResponse {
    fn from(proto: QuoteResponseProto) -> Self {
        Self {
            quote_id: proto.quote_id,
            fee_bps: proto.fee_bps,
            output_amount: proto.output_amount,
            est_fill_time_seconds: proto.est_fill_time_seconds,
            expires_at: proto.expires_at,
            rejected: proto.rejected,
            reject_reason: proto.reject_reason,
            solver_address: proto.solver_address,
            requires_exclusivity: proto.requires_exclusivity,
        }
    }
}
