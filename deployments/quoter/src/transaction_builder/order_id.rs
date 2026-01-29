use alloy::primitives::keccak256;

// Note: This module is no longer used for EVM order ID computation because
// the EVM contract uses block.timestamp for created_at, which cannot be predicted.
// Keeping for reference and potential debugging purposes.
#[allow(dead_code)]
#[derive(Debug, Clone)]
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

#[allow(dead_code)]
impl OrderData {
    /// Encode the order data as bytes for hashing
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = Vec::with_capacity(214);

        encoded.extend_from_slice(&self.version.to_be_bytes());
        encoded.extend_from_slice(&self.sender);
        encoded.extend_from_slice(&self.nonce.to_be_bytes());
        encoded.extend_from_slice(&self.origin_chain_id.to_be_bytes());
        encoded.extend_from_slice(&self.dest_chain_id.to_be_bytes());
        encoded.extend_from_slice(&self.created_at.to_be_bytes());
        encoded.extend_from_slice(&self.fill_deadline.to_be_bytes());
        encoded.extend_from_slice(&self.token_in);
        encoded.extend_from_slice(&self.token_out);
        encoded.extend_from_slice(&self.amount_in.to_be_bytes());
        encoded.extend_from_slice(&self.amount_out.to_be_bytes());
        encoded.extend_from_slice(&self.recipient);
        encoded.extend_from_slice(&self.solver);

        encoded
    }

    pub fn compute_order_id(&self) -> [u8; 32] {
        keccak256(&self.encode()).0
    }
}
