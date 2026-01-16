pub mod error;
pub mod evm;
pub mod order_id;
pub mod svm;

pub use error::TransactionBuilderError;
pub use evm::EvmTransactionBuilder;
pub use svm::SvmTransactionBuilder;

use crate::models::EvmTransaction;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmTransactionResult {
    pub transaction: EvmTransaction,
    pub approval_transaction: Option<EvmTransaction>,
    pub order_id: String,
    pub nonce: u64,
    pub contract_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub transaction: String,
    pub order_id: String,
    pub nonce: u64,
    pub contract_address: String,
}

#[derive(Debug, Clone)]
pub struct OpenOrderInput {
    pub sender_address: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u64,
    pub amount_out: u128,
    pub recipient: [u8; 32],
    pub solver: [u8; 32],
    pub dest_chain_id: u32,
    pub fill_deadline: u64,
}
