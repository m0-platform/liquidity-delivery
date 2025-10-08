use anchor_lang::{prelude::*, solana_program::keccak};

#[repr(u8)]
#[derive(AnchorDeserialize, AnchorSerialize, InitSpace, Clone, PartialEq)]
pub enum OrderStatus {
    DoesNotExist,
    Created,
    CancelRequested,
    Completed,
}

#[repr(u8)]
#[derive(AnchorDeserialize, AnchorSerialize, InitSpace, Clone, PartialEq)]
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
    pub dest_chain_id: u32,
    pub fill_deadline: u64,
    pub nonce: u64,
    pub token_in: Pubkey,
    pub token_out: [u8; 32], // TODO, ok to use Pubkey here?
    pub sender: Pubkey,
    pub recipient: [u8; 32], // TODO, ok to use Pubkey here?
    pub amount_in: u64,
    pub amount_out: u128,
    pub amount_out_filled: u128,
    pub solver: [u8; 32], // TODO, ok to use Pubkey here?
}

#[derive(AnchorDeserialize, AnchorSerialize, Clone, InitSpace)]
pub struct ForeignOrder {
    pub amount_out_filled: u128,
}

// Note: this must match the EVM version exactly
// We derive the Order ID from the hash of this struct
#[derive(Debug, AnchorDeserialize, AnchorSerialize, Clone)]
pub struct OrderData {
    pub version: u16,
    pub origin_chain_id: u32,
    pub sender: [u8; 32], // TODO, ok to use Pubkey here?
    pub nonce: u64,
    pub dest_chain_id: u32,
    pub fill_deadline: u64,
    pub token_out: [u8; 32], // TODO, ok to use Pubkey here?
    pub recipient: [u8; 32], // TODO, ok to use Pubkey here?
    pub amount_out: u128,
    pub solver: [u8; 32], // TODO, ok to use Pubkey here?
}

impl OrderData {
    pub fn compute_order_id(&self) -> [u8; 32] {
        keccak::hashv(&[
            &self.version.to_le_bytes(),
            &self.origin_chain_id.to_le_bytes(),
            &self.sender,
            &self.nonce.to_le_bytes(),
            &self.dest_chain_id.to_le_bytes(),
            &self.fill_deadline.to_le_bytes(),
            &self.token_out,
            &self.recipient,
            &self.amount_out.to_le_bytes(),
            &self.solver,
        ])
        .to_bytes()
    }
}
