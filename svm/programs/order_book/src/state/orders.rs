use anchor_lang::{prelude::*, solana_program::keccak};

use crate::VERSION;

#[repr(u8)]
#[derive(AnchorDeserialize, AnchorSerialize, InitSpace, Clone, Debug, PartialEq)]
pub enum OrderStatus {
    DoesNotExist,
    Created,
    Cancelled,
    Completed,
}

#[repr(u8)]
#[derive(AnchorDeserialize, AnchorSerialize, InitSpace, Clone, Debug, PartialEq)]
pub enum OrderType {
    Native,
    Foreign,
}

#[constant]
pub const ORDER_SEED_PREFIX: &[u8] = b"order";

#[account]
#[derive(InitSpace)]
pub struct Order<T: AnchorDeserialize + AnchorSerialize + Space> {
    pub order_type: OrderType,
    pub bump: u8,
    pub data: T,
}

#[derive(AnchorDeserialize, AnchorSerialize, Clone, InitSpace)]
pub struct NativeOrder {
    pub status: OrderStatus,
    pub version: u16,
    pub sender: Pubkey,
    pub payer: Pubkey,
    pub nonce: u64,
    pub dest_chain_id: u32,
    pub created_at: u64,
    pub fill_deadline: u64,
    pub token_in: Pubkey,
    pub token_out: [u8; 32],
    pub amount_in: u128,
    pub amount_out: u128,
    pub recipient: [u8; 32],
    pub solver: [u8; 32],
    pub amount_in_released: u128,
    pub amount_out_filled: u128,
    pub amount_in_refunded: u128
}

#[derive(AnchorDeserialize, AnchorSerialize, Clone, InitSpace)]
pub struct ForeignOrder {
    pub status: OrderStatus,
    pub amount_in_released: u128,
    pub amount_out_filled: u128,
    pub amount_in_refunded: u128
}

// Note: this must match the EVM version exactly
// We derive the Order ID from the hash of this struct
#[derive(AnchorDeserialize, AnchorSerialize, Clone)]
pub struct OrderData {
    pub version: u16,
    pub sender: [u8; 32],
    pub nonce: u64,
    pub origin_chain_id: u32,
    pub dest_chain_id: u32,
    pub created_at: u64,
    pub fill_deadline: u64,
    pub token_in: [u8; 32],
    pub token_out: [u8; 32],
    pub amount_in: u128,
    pub amount_out: u128,
    pub recipient: [u8; 32],
    pub solver: [u8; 32],
}

// We have both global and struct-level functions for convenience
fn encode_order_data(order_data: &OrderData) -> Vec<u8> {
    let mut encoded: Vec<u8> = vec![];

    encoded.extend_from_slice(&order_data.version.to_be_bytes());
    encoded.extend_from_slice(&order_data.sender);
    encoded.extend_from_slice(&order_data.nonce.to_be_bytes());
    encoded.extend_from_slice(&order_data.origin_chain_id.to_be_bytes());
    encoded.extend_from_slice(&order_data.dest_chain_id.to_be_bytes());
    encoded.extend_from_slice(&order_data.created_at.to_be_bytes());
    encoded.extend_from_slice(&order_data.fill_deadline.to_be_bytes());
    encoded.extend_from_slice(&order_data.token_in);
    encoded.extend_from_slice(&order_data.token_out);
    encoded.extend_from_slice(&order_data.amount_in.to_be_bytes());
    encoded.extend_from_slice(&order_data.amount_out.to_be_bytes());
    encoded.extend_from_slice(&order_data.recipient);
    encoded.extend_from_slice(&order_data.solver);

    encoded
}

pub fn compute_order_id(order_data: &OrderData) -> [u8; 32] {
    keccak::hash(encode_order_data(order_data).as_slice()).to_bytes()
}

impl OrderData {
    pub fn compute_order_id(&self) -> [u8; 32] {
        compute_order_id(&self)
    }

    pub fn encode(&self) -> Vec<u8> {
        encode_order_data(self)
    }

    pub fn new_from_native_order(native_order: NativeOrder, origin_chain_id: u32) -> Self {
        Self {
            version: VERSION,
            sender: native_order.sender.to_bytes(),
            nonce: native_order.nonce,
            origin_chain_id,
            dest_chain_id: native_order.dest_chain_id,
            created_at: native_order.created_at,
            fill_deadline: native_order.fill_deadline,
            token_in: native_order.token_in.to_bytes(),
            token_out: native_order.token_out,
            amount_in: native_order.amount_in,
            amount_out: native_order.amount_out,
            recipient: native_order.recipient,
            solver: native_order.solver,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        primitives::{keccak256, FixedBytes},
        sol,
        sol_types::SolValue,
    };

    sol! {
        IOrderBook,
        "../../../evm/out/IOrderBook.sol/IOrderBook.json"
    }

    #[test]
    fn test_order_id_hash() {
        let evm_order_data = IOrderBook::OrderData {
            version: 1u16,
            sender: FixedBytes::<32>::new([1u8; 32]),
            nonce: 42u64,
            originChainId: 1u32,
            destChainId: 2u32,
            createdAt: 12345500000u64,
            fillDeadline: 1234567890u64,
            tokenIn: FixedBytes::<32>::new([5u8; 32]),
            tokenOut: FixedBytes::<32>::new([2u8; 32]),
            amountIn: 1000u128,
            amountOut: 2000u128,
            recipient: FixedBytes::<32>::new([3u8; 32]),
            solver: FixedBytes::<32>::new([4u8; 32]),
        };

        println!(
            "EVM Version with Packed Encoding: {:?}",
            evm_order_data.abi_encode_packed()
        );

        let expected_hash = keccak256(evm_order_data.abi_encode_packed()).0;

        let order_data = OrderData {
            version: 1u16,
            sender: [1u8; 32],
            nonce: 42u64,
            origin_chain_id: 1u32,
            dest_chain_id: 2u32,
            created_at: 12345500000u64,
            fill_deadline: 1234567890u64,
            token_in: [5u8; 32],
            token_out: [2u8; 32],
            amount_in: 1000u128,
            amount_out: 2000u128,
            recipient: [3u8; 32],
            solver: [4u8; 32],
        };

        println!(
            "SVM Version with Packed Encoding: {:?}",
            encode_order_data(&order_data)
        );

        assert_eq!(order_data.compute_order_id(), expected_hash);
    }
}
