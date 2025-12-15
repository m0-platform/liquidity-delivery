use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use alloy::primitives::Address;
use alloy::providers::Provider;
use anchor_client::solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use anchor_client::solana_client::rpc_filter::{Memcmp, RpcFilterType};
use anchor_client::solana_sdk::commitment_config::CommitmentConfig;
use anchor_client::solana_sdk::pubkey::Pubkey;
use async_trait::async_trait;
use futures_util::future::join_all;
use m0_liquidity_sdk::types::{Asset, Chain, ChainRuntime};
use slog::{debug, error, info, warn, Logger};
use spl_token::solana_program::program_pack::Pack;
use tokio::sync::RwLock;

use crate::components::ComponentParams;
use crate::config::ChainConfig;
use crate::contracts::IERC20;
use crate::error::{Result, SolverError};
use crate::events::{EventHandler, EventProcessor, HoldSuccessfulEvent, SolverEvent};
use crate::providers::ProviderManager;
use crate::stores::{AssetStore, OrderStore};
use crate::utils::{chain_from_id, chain_runtime, format_address};

pub struct InventoryManager {
    asset_store: Arc<RwLock<AssetStore>>,
    order_store: Arc<RwLock<OrderStore>>,
    chains: Vec<ChainConfig>,
    balances: Arc<RwLock<HashMap<Asset, u128>>>,
    holds: Arc<RwLock<HashMap<Chain, HashMap<String, u128>>>>,
    provider_manager: Arc<ProviderManager>,
    logger: Logger,
}

impl InventoryManager {
    pub fn new(params: &ComponentParams) -> Self {
        let logger = params
            .logger
            .new(slog::o!("component" => "InventoryManager"));

        Self {
            asset_store: Arc::new(RwLock::new(AssetStore::new(
                params.config.liquidity_api_url.clone(),
            ))),
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            chains: params.config.chains.clone(),
            balances: Arc::new(RwLock::new(HashMap::new())),
            holds: Arc::new(RwLock::new(HashMap::new())),
            provider_manager: params.provider_manager.clone(),
            logger,
        }
    }

    async fn load_svm_balances(&self) -> Result<()> {
        let address = self.provider_manager.svm_address;

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
                .await;

            debug!(
                self.logger,
                "Loading SVM balances";
                "chain_id" => %chain.chain_id,
                "asset_count" => assets.len(),
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
                                    debug!(
                                        self.logger,
                                        "Found svm token balance";
                                        "chain_id" => %chain.chain_id,
                                        "ata" => %pubkey,
                                        "asset" => &asset.symbol,
                                        "amount" => amount,
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            self.logger,
                            "Failed to fetch token accounts for program";
                            "chain_id" => %chain.chain_id,
                            "token_program" => token_program.to_string(),
                            "error" => %e,
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
                    error!(
                        self.logger,
                        "Failed to load native SOL balance";
                        "chain_id" => %chain.chain_id,
                        "error" => %e,
                    );
                }
            }
        }

        Ok(())
    }

    async fn load_evm_balances(&self) -> Result<()> {
        let address = self.provider_manager.evm_address;

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
                .await;

            debug!(
                self.logger,
                "Loading EVM balances";
                "chain_id" => %chain.chain_id,
                "asset_count" => assets.len(),
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

                    async move {
                        // Parse the token address from hex string
                        let token_address = match Address::from_str(&asset.address) {
                            Ok(addr) => addr,
                            Err(_e) => {
                                return 0;
                            }
                        };

                        // Get the provider with rate limiting
                        let provider = provider_wrapper.provider().await;
                        let token = IERC20::new(token_address, provider);

                        match token.balanceOf(address).call().await {
                            Ok(balance) => balance.to::<u128>(),
                            Err(_e) => 0,
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
                    error!(
                        self.logger,
                        "Failed to load native ETH balance";
                        "chain_id" => %chain.chain_id,
                        "error" => %e,
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
                    "{} ({}): {}",
                    asset.symbol,
                    asset.chain,
                    *balance as f64 / 10_f64.powf(asset.decimals as f64)
                )
            })
            .collect();

        info!(
            self.logger,
            "Current signer balances";
            "balances" => ?balance_info,
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
        self.order_store.write().await.initialize().await?;

        // Load balances before starting
        self.load_svm_balances().await?;
        self.load_evm_balances().await?;
        self.log_balances().await;

        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let store = self.order_store.read().await;
        let _ = store.handle_event(event.clone()).await;

        match event {
            SolverEvent::RequestHold(e) => {
                let active_holds = self
                    .holds
                    .read()
                    .await
                    .get(&e.asset.chain)
                    .and_then(|h| h.get(&e.asset.address).cloned())
                    .unwrap_or(0);

                let current_balance = self
                    .balances
                    .read()
                    .await
                    .get(&e.asset)
                    .cloned()
                    .unwrap_or(0);

                if current_balance - active_holds >= e.amount {
                    let mut holds = self.holds.write().await;

                    holds
                        .entry(e.asset.chain)
                        .or_insert_with(HashMap::new)
                        .insert(e.asset.address.clone(), active_holds + e.amount);

                    return Ok(vec![SolverEvent::HoldSuccessful(HoldSuccessfulEvent::new(
                        e.order_id, e.amount,
                    ))]);
                } else {
                    // TODO: acquire inventory
                }
            }
            SolverEvent::OrderCompleted(e) => {
                // Release holds associated with the order
                let order = self.order_store.read().await.get_order(&e.order_id).await?;

                let address = format_address(&order.data.token_out);

                let mut holds = self.holds.write().await;
                let chain_holds = holds
                    .entry(chain_from_id(order.data.dest_chain_id))
                    .or_insert_with(HashMap::new);

                chain_holds.insert(
                    address.clone(),
                    chain_holds
                        .get(&address)
                        .cloned()
                        .unwrap_or(0)
                        .saturating_sub(order.data.amount_in),
                );
            }
            // TODO: release funds on OrderRejected, OrderCancelRequest, etc.
            _ => {}
        }

        Ok(vec![])
    }
}
