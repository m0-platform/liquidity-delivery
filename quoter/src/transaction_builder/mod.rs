pub mod error;
pub mod evm;
pub mod order_id;
pub mod svm;

pub use error::TransactionBuilderError;
pub use evm::EvmTransactionBuilder;
pub use svm::SvmTransactionBuilder;

use crate::models::EvmTransaction;
use serde::{Deserialize, Serialize};

/// Result of building an EVM transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmTransactionResult {
    /// The open order transaction
    pub transaction: EvmTransaction,
    /// Approval transaction if allowance is insufficient (None if not needed)
    pub approval_transaction: Option<EvmTransaction>,
    /// The computed order ID (hex, 32 bytes, with 0x prefix)
    pub order_id: String,
    /// The nonce used in this order
    pub nonce: u64,
    /// The contract/program address
    pub contract_address: String,
}

/// Result of building a transaction (legacy for SVM compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    /// The serialized transaction (base64 for SVM, hex for EVM calldata)
    pub transaction: String,
    /// The computed order ID (hex, 32 bytes, with 0x prefix)
    pub order_id: String,
    /// The nonce used in this order
    pub nonce: u64,
    /// The contract/program address
    pub contract_address: String,
}

/// Input parameters for building an open order transaction
#[derive(Debug, Clone)]
pub struct OpenOrderInput {
    /// Sender wallet address (EVM hex or Solana base58)
    pub sender_address: String,
    /// Input token address on origin chain
    pub token_in: String,
    /// Output token address on destination chain (hex or base58)
    pub token_out: String,
    /// Amount of input tokens
    pub amount_in: u64,
    /// Expected amount of output tokens
    pub amount_out: u128,
    /// Recipient address on destination chain (as bytes32)
    pub recipient: [u8; 32],
    /// Solver address (as bytes32), or all zeros for any solver
    pub solver: [u8; 32],
    /// Destination chain ID
    pub dest_chain_id: u32,
    /// Fill deadline (Unix timestamp)
    pub fill_deadline: u64,
}

impl OpenOrderInput {
    /// Create a new OpenOrderInput with default recipient (same as sender) and no solver restriction
    pub fn new(
        sender_address: String,
        token_in: String,
        token_out: String,
        amount_in: u64,
        amount_out: u128,
        dest_chain_id: u32,
        fill_deadline: u64,
    ) -> Self {
        Self {
            sender_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            recipient: [0u8; 32], // Default to zero, caller should set
            solver: [0u8; 32],    // No solver restriction by default
            dest_chain_id,
            fill_deadline,
        }
    }

    /// Set the recipient address from a string (hex or base58)
    pub fn with_recipient(mut self, recipient: [u8; 32]) -> Self {
        self.recipient = recipient;
        self
    }

    /// Set the solver address from a string (hex or base58)
    pub fn with_solver(mut self, solver: [u8; 32]) -> Self {
        self.solver = solver;
        self
    }
}
