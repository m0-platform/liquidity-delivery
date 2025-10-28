use alloy::{
    network::Ethereum,
    providers::{
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
        Identity, ProviderBuilder, RootProvider,
    },
};
use anchor_client::solana_client::rpc_client::RpcClient;
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use m0_liquidity_sdk::types::ChainRuntime;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::ChainConfig;
use crate::error::{Result, SolverError};
use crate::utils::chain_runtime;

/// Type for the EVM provider
type EvmProviderInner = FillProvider<
    JoinFill<
        Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    RootProvider<Ethereum>,
>;

/// Wrapper around EVM provider with rate limiting
pub struct EvmProvider {
    provider: EvmProviderInner,
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl EvmProvider {
    /// Get a reference to the provider, waiting for rate limit if needed
    pub async fn provider(&self) -> &EvmProviderInner {
        // Wait until rate limiter allows the request
        self.rate_limiter.until_ready().await;
        &self.provider
    }

    /// Get a reference to the provider (bypasses rate limiting)
    pub fn priority_provider(&self) -> &EvmProviderInner {
        &self.provider
    }
}

/// Wrapper around SVM RPC client with rate limiting
pub struct SvmProvider {
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
    pub fn priority_client(&self) -> &RpcClient {
        &self.client
    }
}

/// Global provider manager that maintains rate-limited providers for all chains
pub struct ProviderManager {
    evm_providers: Arc<RwLock<HashMap<u32, Arc<EvmProvider>>>>,
    svm_providers: Arc<RwLock<HashMap<u32, Arc<SvmProvider>>>>,
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl ProviderManager {
    /// Create a new provider manager
    ///
    /// # Arguments
    /// * `max_requests_per_second` - Maximum sustained requests per second across all chains
    /// * `burst_size` - Maximum burst capacity
    pub fn new(max_requests_per_second: u32, burst_size: u32) -> Self {
        // Create quota: burst_size tokens that refill at max_requests_per_second rate
        let quota = Quota::per_second(NonZeroU32::new(max_requests_per_second).unwrap())
            .allow_burst(NonZeroU32::new(burst_size).unwrap());

        let rate_limiter = RateLimiter::direct(quota);

        Self {
            evm_providers: Arc::new(RwLock::new(HashMap::new())),
            svm_providers: Arc::new(RwLock::new(HashMap::new())),
            rate_limiter: Arc::new(rate_limiter),
        }
    }

    /// Initialize providers for all configured chains
    pub async fn initialize(&self, chains: &[ChainConfig]) -> Result<()> {
        for chain in chains {
            match chain_runtime(chain.chain_id) {
                ChainRuntime::Evm => {
                    self.add_evm_provider(chain).await?;
                }
                ChainRuntime::Svm => {
                    self.add_svm_provider(chain).await?;
                }
            }
        }

        tracing::info!(
            evm_chains = self.evm_providers.read().await.len(),
            svm_chains = self.svm_providers.read().await.len(),
            "Initialized provider manager"
        );

        Ok(())
    }

    /// Add an EVM provider for a chain
    async fn add_evm_provider(&self, chain: &ChainConfig) -> Result<()> {
        let url = chain.rpc_url.parse().map_err(|e| {
            SolverError::Component(format!(
                "Invalid RPC URL for chain {}: {}",
                chain.chain_id, e
            ))
        })?;

        let provider = ProviderBuilder::new().connect_http(url);

        let evm_provider = Arc::new(EvmProvider {
            provider,
            rate_limiter: self.rate_limiter.clone(),
        });

        self.evm_providers
            .write()
            .await
            .insert(chain.chain_id, evm_provider);

        tracing::debug!(
            chain_id = chain.chain_id,
            rpc_url = chain.rpc_url,
            "Added EVM provider"
        );

        Ok(())
    }

    /// Add an SVM provider for a chain
    async fn add_svm_provider(&self, chain: &ChainConfig) -> Result<()> {
        let client = Arc::new(RpcClient::new(&chain.rpc_url));

        let svm_provider = Arc::new(SvmProvider {
            client,
            rate_limiter: self.rate_limiter.clone(),
        });

        self.svm_providers
            .write()
            .await
            .insert(chain.chain_id, svm_provider);

        tracing::debug!(
            chain_id = chain.chain_id,
            rpc_url = chain.rpc_url,
            "Added SVM provider"
        );

        Ok(())
    }

    /// Get an EVM provider for a chain
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

    /// Get an SVM provider for a chain
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
