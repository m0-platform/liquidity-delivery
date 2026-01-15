use alloy::primitives::keccak256;

#[derive(Debug, Clone)]
pub struct OrderData {
    pub version: u16,
    pub sender: [u8; 32],
    pub nonce: u64,
    pub origin_chain_id: u32,
    pub dest_chain_id: u32,
    pub fill_deadline: u64,
    pub token_in: [u8; 32],
    pub token_out: [u8; 32],
    pub amount_in: u128,
    pub amount_out: u128,
    pub recipient: [u8; 32],
    pub solver: [u8; 32],
}

impl OrderData {
    /// Encode the order data as bytes for hashing
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = Vec::with_capacity(206);

        encoded.extend_from_slice(&self.version.to_be_bytes());
        encoded.extend_from_slice(&self.sender);
        encoded.extend_from_slice(&self.nonce.to_be_bytes());
        encoded.extend_from_slice(&self.origin_chain_id.to_be_bytes());
        encoded.extend_from_slice(&self.dest_chain_id.to_be_bytes());
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
