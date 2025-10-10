use alloy::signers::k256::sha2::{Digest, Sha256};

pub enum EventDiscriminator {
    OrderOpened,
}

impl EventDiscriminator {
    pub fn discriminator(&self) -> [u8; 8] {
        match self {
            EventDiscriminator::OrderOpened => event_hash("OrderOpened"),
        }
    }
}

fn event_hash(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("event:{name}"));
    let result = hasher.finalize();
    result[..8].try_into().unwrap()
}
