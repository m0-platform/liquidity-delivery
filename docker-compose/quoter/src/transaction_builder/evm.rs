use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::ProviderBuilder;
use alloy::sol;
use alloy::sol_types::SolCall;
use std::str::FromStr;

use super::error::TransactionBuilderError;
use super::order_id::OrderData;
use super::{EvmTransactionResult, OpenOrderInput};
use crate::models::EvmTransaction;

sol! {
    #[sol(rpc)]
    interface IOrderBook {
        struct OrderParams {
            uint32 destChainId;
            uint32 fillDeadline;
            address tokenIn;
            bytes32 tokenOut;
            uint128 amountIn;
            uint128 amountOut;
            bytes32 recipient;
            bytes32 solver;
        }

        function openOrder(OrderParams calldata orderParams_) external returns (bytes32);
        function getSenderNonce(address sender_) external view returns (uint64);
    }
}

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

pub struct EvmTransactionBuilder {
    rpc_url: String,
    contract_address: Address,
    chain_id: u32,
}

impl EvmTransactionBuilder {
    pub fn new(
        rpc_url: String,
        contract_address: String,
        chain_id: u32,
    ) -> Result<Self, TransactionBuilderError> {
        let address = Address::from_str(&contract_address)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;
        Ok(Self {
            rpc_url,
            contract_address: address,
            chain_id,
        })
    }

    /// Fetch the current nonce for a sender from the contract
    pub async fn get_sender_nonce(&self, sender: &str) -> Result<u64, TransactionBuilderError> {
        let rpc_url: url::Url = self
            .rpc_url
            .parse()
            .map_err(|e| TransactionBuilderError::RpcError(format!("Invalid RPC URL: {}", e)))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url);
        let sender_addr = Address::from_str(sender)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;

        let contract = IOrderBook::new(self.contract_address, &provider);
        let result = contract
            .getSenderNonce(sender_addr)
            .call()
            .await
            .map_err(|e| TransactionBuilderError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// Check the current ERC20 allowance for a token
    pub async fn get_allowance(
        &self,
        token: &str,
        owner: &str,
        spender: &str,
    ) -> Result<U256, TransactionBuilderError> {
        let rpc_url: url::Url = self
            .rpc_url
            .parse()
            .map_err(|e| TransactionBuilderError::RpcError(format!("Invalid RPC URL: {}", e)))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url);

        let token_addr = Address::from_str(token)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;
        let owner_addr = Address::from_str(owner)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;
        let spender_addr = Address::from_str(spender)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;

        let contract = IERC20::new(token_addr, &provider);
        let result = contract
            .allowance(owner_addr, spender_addr)
            .call()
            .await
            .map_err(|e| TransactionBuilderError::RpcError(e.to_string()))?;

        Ok(result)
    }

    pub fn build_approve_calldata(
        token: &str,
        spender: &str,
        amount: u128,
    ) -> Result<EvmTransaction, TransactionBuilderError> {
        let spender_addr = Address::from_str(spender)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;

        let calldata = IERC20::approveCall {
            spender: spender_addr,
            amount: U256::from(amount),
        }
        .abi_encode();

        Ok(EvmTransaction {
            to: token.to_string(),
            data: format!("0x{}", hex::encode(&calldata)),
            value: "0x0".to_string(),
        })
    }

    /// Build openOrder calldata
    pub async fn build_open_order_calldata(
        &self,
        input: &OpenOrderInput,
    ) -> Result<EvmTransactionResult, TransactionBuilderError> {
        let nonce = self.get_sender_nonce(&input.sender_address).await?;

        // Parse addresses
        let token_in = Address::from_str(&input.token_in)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;
        let sender = Address::from_str(&input.sender_address)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;

        // Parse token_out as bytes32 (could be Solana address or EVM address)
        let token_out = parse_bytes32(&input.token_out)?;

        // Check current allowance and build approval tx if needed
        let current_allowance = self
            .get_allowance(
                &input.token_in,
                &input.sender_address,
                &format!("{:?}", self.contract_address),
            )
            .await?;

        let approval_transaction = if current_allowance < U256::from(input.amount_in) {
            // Build approval for max uint256 to avoid repeated approvals
            Some(Self::build_approve_calldata(
                &input.token_in,
                &format!("{:?}", self.contract_address),
                u128::MAX,
            )?)
        } else {
            None
        };

        let order_params = IOrderBook::OrderParams {
            destChainId: input.dest_chain_id,
            fillDeadline: input.fill_deadline as u32,
            tokenIn: token_in,
            tokenOut: token_out,
            amountIn: input.amount_in as u128,
            amountOut: input.amount_out,
            recipient: FixedBytes::from(input.recipient),
            solver: FixedBytes::from(input.solver),
        };

        let calldata = IOrderBook::openOrderCall {
            orderParams_: order_params,
        }
        .abi_encode();

        // Convert sender address to bytes32 (left-padded)
        let mut sender_bytes32 = [0u8; 32];
        sender_bytes32[12..].copy_from_slice(sender.as_slice());

        // Convert token_in to bytes32 (left-padded)
        let mut token_in_bytes32 = [0u8; 32];
        token_in_bytes32[12..].copy_from_slice(token_in.as_slice());

        // Compute order ID
        let order_data = OrderData {
            version: 1, // VERSION constant
            sender: sender_bytes32,
            nonce,
            origin_chain_id: self.chain_id,
            dest_chain_id: input.dest_chain_id,
            fill_deadline: input.fill_deadline,
            token_in: token_in_bytes32,
            token_out: token_out.0,
            amount_in: input.amount_in as u128,
            amount_out: input.amount_out,
            recipient: input.recipient,
            solver: input.solver,
        };
        let order_id = order_data.compute_order_id();

        Ok(EvmTransactionResult {
            transaction: EvmTransaction {
                to: format!("{:?}", self.contract_address),
                data: format!("0x{}", hex::encode(&calldata)),
                value: "0x0".to_string(),
            },
            approval_transaction,
            order_id: format!("0x{}", hex::encode(order_id)),
            nonce,
            contract_address: format!("{:?}", self.contract_address),
        })
    }
}

/// Parse a string as bytes32, supporting various formats
fn parse_bytes32(s: &str) -> Result<FixedBytes<32>, TransactionBuilderError> {
    // Handle hex strings (EVM addresses with or without 0x prefix)
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() == 40 {
        // EVM address - left-pad with zeros
        let mut bytes = [0u8; 32];
        let addr_bytes =
            hex::decode(s).map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;
        bytes[12..].copy_from_slice(&addr_bytes);
        return Ok(FixedBytes::from(bytes));
    }
    if s.len() == 64 {
        // Full bytes32
        let bytes: [u8; 32] = hex::decode(s)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?
            .try_into()
            .map_err(|_| TransactionBuilderError::InvalidAddress("Invalid length".to_string()))?;
        return Ok(FixedBytes::from(bytes));
    }

    // Try base58 (Solana pubkey)
    let decoded = bs58::decode(s)
        .into_vec()
        .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;
    if decoded.len() == 32 {
        let bytes: [u8; 32] = decoded
            .try_into()
            .map_err(|_| TransactionBuilderError::InvalidAddress("Invalid length".to_string()))?;
        return Ok(FixedBytes::from(bytes));
    }

    Err(TransactionBuilderError::InvalidAddress(format!(
        "Cannot parse: {}",
        s
    )))
}
