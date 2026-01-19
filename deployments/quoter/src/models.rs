use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub input_token: String,
    pub input_chain_id: u32,
    pub output_token: String,
    pub output_chain_id: u32,
    pub amount_in: u64,
    /// Sender address for transaction building (optional)
    #[serde(default)]
    pub sender_address: Option<String>,
    /// Recipient address on destination chain (defaults to sender if not provided)
    #[serde(default)]
    pub recipient: Option<String>,
}

/// EVM transaction parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmTransaction {
    /// Target contract address (hex with 0x prefix)
    pub to: String,
    /// Transaction calldata (hex with 0x prefix)
    pub data: String,
    /// Transaction value in wei (hex with 0x prefix)
    pub value: String,
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
    /// Computed order ID (hex string) for redirect after order creation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    /// EVM transaction to open the order
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evm_transaction: Option<EvmTransaction>,
    /// EVM approval transaction (if token allowance is insufficient)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_transaction: Option<EvmTransaction>,
    /// Serialized SVM transaction (base64)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svm_transaction: Option<String>,
    /// Nonce used for the order
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<u64>,
    /// OrderBook contract address for the input chain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orderbook_address: Option<String>,
}
