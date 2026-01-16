use std::sync::Arc;

use alloy::{primitives::Address, providers::ProviderBuilder, signers::local::PrivateKeySigner};
use anchor_client::solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

use crate::error::{Result, SolverError};
use crate::providers::EvmFillProvider;

#[derive(Clone)]
pub struct Signers {
    evm_private_key: PrivateKeySigner,
    svm_private_key: Arc<Keypair>,
}

impl Signers {
    pub fn new(evm_private_key: PrivateKeySigner, svm_private_key: Arc<Keypair>) -> Self {
        Signers {
            evm_private_key,
            svm_private_key,
        }
    }

    pub fn svm_address(&self) -> Pubkey {
        self.svm_private_key.pubkey()
    }

    pub fn svm_keypair(&self) -> Arc<Keypair> {
        self.svm_private_key.clone()
    }

    pub fn evm_address(&self) -> Address {
        self.evm_private_key.address()
    }

    pub fn evm_provider(&self, url: String) -> Result<EvmFillProvider> {
        let url = url
            .parse()
            .map_err(|e| SolverError::Component(format!("Invalid RPC URL {}: {}", url, e)))?;

        let provider = ProviderBuilder::new()
            .wallet(self.evm_private_key.clone())
            .connect_http(url);

        Ok(provider)
    }
}

impl Default for Signers {
    fn default() -> Self {
        Signers {
            evm_private_key: PrivateKeySigner::random(),
            svm_private_key: Arc::new(Keypair::new()),
        }
    }
}
