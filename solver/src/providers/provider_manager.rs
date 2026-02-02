use alloy::primitives::Address;
use anchor_client::{
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey},
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use m0_liquidity_sdk::types::ChainRuntime;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{config::ChainConfig, providers::EvmFillProvider};
use crate::{
    error::{Result, SolverError},
    providers::Signers,
};
use crate::{utils::chain_runtime, Config};

/// Wrapper around EVM provider with rate limiting
pub struct EvmProvider {
    provider: EvmFillProvider,
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl EvmProvider {
    /// Get a reference to the provider, waiting for rate limit if needed
    pub async fn provider(&self) -> &EvmFillProvider {
        // Wait until rate limiter allows the request
        self.rate_limiter.until_ready().await;
        &self.provider
    }

    /// Get a reference to the provider (bypasses rate limiting)
    #[allow(dead_code)]
    pub fn priority_provider(&self) -> &EvmFillProvider {
        &self.provider
    }
}

/// Wrapper around SVM RPC client with rate limiting
pub struct SvmProvider {
    pub pubsub_client: Arc<PubsubClient>,
    client: Arc<RpcClient>,
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl SvmProvider {
    /// Get a reference to the RPC client with rate limiting
    pub async fn client(&self) -> &RpcClient {
        self.rate_limiter.until_ready().await;
        &self.client
    }

    /// Get a reference to the underlying client (bypasses rate limiting)
    #[allow(dead_code)]
    pub fn priority_client(&self) -> &RpcClient {
        &self.client
    }
}

/// Global provider manager that maintains rate-limited providers for all chains
pub struct ProviderManager {
    evm_providers: Arc<RwLock<HashMap<u32, Arc<EvmProvider>>>>,
    svm_providers: Arc<RwLock<HashMap<u32, Arc<SvmProvider>>>>,
    rate_limiter_quota: Quota,
    pub svm_address: Pubkey,
    pub evm_address: Address,
}

impl ProviderManager {
    pub fn new(config: &Config) -> Self {
        let rpc_cfg = &config.rpc_rate_limit;
        let quota = Quota::per_second(NonZeroU32::new(rpc_cfg.max_requests_per_second).unwrap())
            .allow_burst(NonZeroU32::new(rpc_cfg.burst_size).unwrap());

        Self {
            evm_providers: Arc::new(RwLock::new(HashMap::new())),
            svm_providers: Arc::new(RwLock::new(HashMap::new())),
            rate_limiter_quota: quota,
            svm_address: config.signers.svm_address(),
            evm_address: config.signers.evm_address(),
        }
    }

    pub async fn initialize(&self, chains: &[ChainConfig], signers: &Signers) -> Result<()> {
        for chain in chains {
            match chain_runtime(chain.chain_id) {
                ChainRuntime::Evm => {
                    self.add_evm_provider(chain, signers).await?;
                }
                ChainRuntime::Svm => {
                    self.add_svm_provider(chain, signers).await?;
                }
            }
        }

        Ok(())
    }

    async fn add_evm_provider(&self, chain: &ChainConfig, signers: &Signers) -> Result<()> {
        self.evm_providers.write().await.insert(
            chain.chain_id,
            Arc::new(EvmProvider {
                provider: signers.evm_provider(chain.rpc_url.clone())?,
                rate_limiter: Arc::new(RateLimiter::direct(self.rate_limiter_quota)),
            }),
        );

        Ok(())
    }

    async fn add_svm_provider(&self, chain: &ChainConfig, _signers: &Signers) -> Result<()> {
        let client = Arc::new(RpcClient::new_with_commitment(
            chain.rpc_url.clone(),
            CommitmentConfig::confirmed(),
        ));
        let limiter = Arc::new(RateLimiter::direct(self.rate_limiter_quota));

        let pubsub_client = Arc::new(PubsubClient::new(&chain.ws_url).await.map_err(|e| {
            SolverError::Component(format!(
                "Failed to create PubsubClient ({}): {}",
                chain.ws_url, e
            ))
        })?);

        let svm_provider = Arc::new(SvmProvider {
            client,
            pubsub_client,
            rate_limiter: limiter,
        });

        self.svm_providers
            .write()
            .await
            .insert(chain.chain_id, svm_provider);

        Ok(())
    }

    pub async fn get_evm_provider(&self, chain_id: u32) -> Result<Arc<EvmProvider>> {
        self.evm_providers
            .read()
            .await
            .get(&chain_id)
            .cloned()
            .ok_or_else(|| {
                SolverError::Component(format!("No EVM provider found for chain {}", chain_id))
            })
    }

    pub async fn get_svm_provider(&self, chain_id: u32) -> Result<Arc<SvmProvider>> {
        self.svm_providers
            .read()
            .await
            .get(&chain_id)
            .cloned()
            .ok_or_else(|| {
                SolverError::Component(format!("No SVM provider found for chain {}", chain_id))
            })
    }
}
