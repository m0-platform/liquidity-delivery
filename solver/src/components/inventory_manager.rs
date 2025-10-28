use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::sol;
use anchor_client::solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use anchor_client::solana_client::rpc_filter::{Memcmp, RpcFilterType};
use anchor_client::solana_sdk::commitment_config::CommitmentConfig;
use anchor_client::solana_sdk::pubkey::Pubkey;
use async_trait::async_trait;
use futures_util::future::join_all;
use m0_liquidity_sdk::types::{Asset, ChainRuntime};
use spl_token::solana_program::program_pack::Pack;
use tokio::sync::RwLock;

use crate::config::{ChainConfig, Signers};
use crate::error::{Result, SolverError};
use crate::events::{EventHandler, EventProcessor, SolverEvent};
use crate::providers::ProviderManager;
use crate::stores::AssetStore;
use crate::utils::chain_runtime;
use crate::Config;

// Define ERC20 interface for balance checking
sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function decimals() external view returns (uint8);
    }
}

pub struct InventoryManager {
    signers: Signers,
    asset_store: Arc<RwLock<AssetStore>>,
    chains: Vec<ChainConfig>,
    balances: Arc<RwLock<HashMap<Asset, u128>>>,
    provider_manager: Arc<ProviderManager>,
}

impl InventoryManager {
    pub fn new(cfg: Config, provider_manager: Arc<ProviderManager>) -> Self {
        Self {
            signers: cfg.signers,
            asset_store: Arc::new(RwLock::new(AssetStore::new(cfg.liquidity_api_url))),
            chains: cfg.chains,
            balances: Arc::new(RwLock::new(HashMap::new())),
            provider_manager,
        }
    }

    async fn load_svm_balances(&self) -> Result<()> {
        let address = self.signers.svm_address();

        // Get all SVM chains
        let svm_chains: Vec<_> = self
            .chains
            .iter()
            .filter(|chain| chain_runtime(chain.chain_id) == ChainRuntime::Svm)
            .collect();

        for chain in svm_chains {
            let assets = self
                .asset_store
                .read()
                .await
                .get_assets_for_chain(chain.chain_id)
                .await?;

            tracing::debug!(
                chain_id = chain.chain_id,
                asset_count = assets.len(),
                "Loading SVM balances"
            );

            let provider = self
                .provider_manager
                .get_svm_provider(chain.chain_id)
                .await?;

            let client = provider.client().await;

            // Create a map of mint addresses to assets for quick lookup
            let mut mint_to_asset: HashMap<Pubkey, Asset> = HashMap::new();
            for asset in assets.iter() {
                if let Ok(mint_pubkey) = Pubkey::from_str(&asset.address) {
                    mint_to_asset.insert(mint_pubkey, asset.clone());
                }
            }

            for token_program in [
                spl_token::ID,
                Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap(),
            ] {
                let config = RpcProgramAccountsConfig {
                    filters: Some(vec![
                        // Filter by owner
                        RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                            32,
                            address.to_bytes().to_vec(),
                        )),
                        // Filter by data length (165 bytes for token accounts)
                        RpcFilterType::DataSize(165),
                    ]),
                    account_config: RpcAccountInfoConfig {
                        encoding: None,
                        data_slice: None,
                        commitment: Some(CommitmentConfig::confirmed()),
                        min_context_slot: None,
                    },
                    with_context: Some(false),
                    sort_results: None,
                };

                match client.get_program_accounts_with_config(&token_program, config) {
                    Ok(accounts) => {
                        for (pubkey, account) in accounts {
                            let account = spl_token::state::Account::unpack(&account.data[..]);

                            let (amount, mint) = match account {
                                Ok(acc) => (acc.amount as u128, acc.mint),
                                Err(_) => continue,
                            };

                            if amount > 0 {
                                if let Some(asset) = mint_to_asset.get(&mint) {
                                    self.balances.write().await.insert(asset.clone(), amount);
                                    tracing::debug!(
                                        chain_id = chain.chain_id,
                                        ata = %pubkey,
                                        asset = asset.symbol,
                                        amount = amount,
                                        "Found svm token balance"
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            chain_id = chain.chain_id,
                            token_program = token_program.to_string(),
                            error = %e,
                            "Failed to fetch token accounts for program"
                        );
                    }
                }
            }

            // Also get native SOL balance
            let sol_balance_result = client.get_balance(&address);

            match sol_balance_result {
                Ok(lamports) => {
                    self.balances
                        .write()
                        .await
                        .insert(AssetStore::get_native(chain.chain), lamports.into());
                }
                Err(e) => {
                    tracing::error!(
                        chain_id = chain.chain_id,
                        error = %e,
                        "Failed to load native SOL balance"
                    );
                }
            }
        }

        Ok(())
    }

    async fn load_evm_balances(&self) -> Result<()> {
        let address_str = self.signers.evm_address();
        let address = Address::from_str(&address_str)
            .map_err(|e| SolverError::Component(format!("Invalid EVM address: {}", e)))?;

        // Get all EVM chains
        let evm_chains: Vec<_> = self
            .chains
            .iter()
            .filter(|chain| chain_runtime(chain.chain_id) == ChainRuntime::Evm)
            .collect();

        for chain in evm_chains {
            let assets = self
                .asset_store
                .read()
                .await
                .get_assets_for_chain(chain.chain_id)
                .await?;

            tracing::debug!(
                chain_id = chain.chain_id,
                asset_count = assets.len(),
                "Loading EVM balances"
            );

            let provider_wrapper = self
                .provider_manager
                .get_evm_provider(chain.chain_id)
                .await?;

            // Collect all balance futures
            let balance_futures: Vec<_> = assets
                .iter()
                .map(|asset| {
                    let asset = asset.clone();
                    let provider_wrapper = provider_wrapper.clone();
                    let chain_id = chain.chain_id;

                    async move {
                        // Parse the token address from hex string
                        let token_address = match Address::from_str(&asset.address) {
                            Ok(addr) => addr,
                            Err(e) => {
                                tracing::warn!(
                                    chain_id = chain_id,
                                    asset = ?asset.symbol,
                                    error = %e,
                                    "Invalid token address"
                                );
                                return 0;
                            }
                        };

                        // Get the provider with rate limiting
                        let provider = provider_wrapper.provider().await;
                        let token = IERC20::new(token_address, provider);

                        match token.balanceOf(address).call().await {
                            Ok(balance) => balance.to::<u128>(),
                            Err(e) => {
                                tracing::warn!(
                                    chain_id = chain_id,
                                    asset = ?asset.symbol,
                                    token_address = %asset.address,
                                    error = %e,
                                    "Failed to load ERC20 token balance"
                                );
                                0
                            }
                        }
                    }
                })
                .collect();

            for (asset, amount) in assets.iter().zip(join_all(balance_futures).await.iter()) {
                if *amount > 0 {
                    self.balances.write().await.insert(asset.clone(), *amount);
                }
            }

            // Also get native ETH balance
            let provider = provider_wrapper.provider().await;
            let eth_balance_result = provider
                .get_balance(address)
                .await
                .map_err(|e| SolverError::Component(format!("Failed to get balance: {}", e)));

            match eth_balance_result {
                Ok(balance) => {
                    self.balances
                        .write()
                        .await
                        .insert(AssetStore::get_native(chain.chain), balance.to());
                }
                Err(e) => {
                    tracing::error!(
                        chain_id = chain.chain_id,
                        error = %e,
                        "Failed to load native ETH balance"
                    );
                }
            }
        }

        Ok(())
    }

    async fn log_balances(&self) {
        let balances = self.balances.read().await;

        let balance_info: Vec<String> = balances
            .iter()
            .map(|(asset, balance)| {
                format!(
                    "{}: {}",
                    asset.symbol,
                    *balance as f64 / 10_f64.powf(asset.decimals as f64)
                )
            })
            .collect();

        tracing::info!(
            balances = ?balance_info,
            "Current signer balances"
        );
    }
}

#[async_trait]
impl EventHandler for InventoryManager {
    fn name(&self) -> &'static str {
        "InventoryManager"
    }

    async fn initialize(&self) -> Result<()> {
        self.asset_store.write().await.initialize().await?;

        // Load balances before starting
        self.load_svm_balances().await?;
        self.load_evm_balances().await?;
        self.log_balances().await;

        Ok(())
    }

    async fn handle_event(&self, _event: SolverEvent) -> Result<Vec<SolverEvent>> {
        Ok(vec![])
    }
}
