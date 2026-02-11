use std::cmp::min;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use alloy::primitives::Address;
use alloy::providers::Provider;
use anchor_client::solana_account_decoder::UiAccountData;
use anchor_client::solana_sdk::pubkey::Pubkey;
use async_trait::async_trait;
use futures_util::future::join_all;
use m0_liquidity_sdk::types::{Asset, Chain, ChainRuntime};
use slog::{debug, error, info, warn, Logger};
use solana_client::rpc_request::TokenAccountsFilter;
use tokio::sync::Mutex;

use crate::components::ComponentParams;
use crate::config::ChainConfig;
use crate::contracts::IERC20;
use crate::error::{Result, SolverError};
use crate::events::{
    EventHandler, EventProcessor, HoldSuccessfulEvent, InventoryUpdateEvent, RequestSwapEvent,
    SolverEvent,
};
use crate::providers::ProviderManager;
use crate::stores::{AssetStore, OrderStore};
use crate::utils::{chain_from_id, chain_runtime, format_address};

const BALANCE_RELOAD_INTERVAL: Duration = Duration::from_secs(2 * 60);

pub struct InventoryManager {
    asset_store: Arc<AssetStore>,
    order_store: Arc<OrderStore>,
    chains: Vec<ChainConfig>,
    balances: Arc<Mutex<HashMap<Asset, u128>>>,
    holds: Arc<Mutex<HashMap<Chain, HashMap<String, u128>>>>,
    order_holds: Arc<Mutex<HashMap<String, u128>>>,
    auto_rebalance: bool,
    provider_manager: Arc<ProviderManager>,
    last_balance_reload: Arc<Mutex<Instant>>,
    logger: Logger,
}

impl InventoryManager {
    pub fn new(params: &ComponentParams) -> Self {
        let logger = params
            .logger
            .new(slog::o!("component" => "InventoryManager"));

        Self {
            asset_store: Arc::new(AssetStore::new(params.config.liquidity_api_url.clone())),
            order_store: Arc::new(OrderStore::new()),
            chains: params.config.chains.clone(),
            balances: Arc::new(Mutex::new(HashMap::new())),
            holds: Arc::new(Mutex::new(HashMap::new())),
            order_holds: Arc::new(Mutex::new(HashMap::new())),
            auto_rebalance: params.config.auto_rebalance,
            provider_manager: params.provider_manager.clone(),
            last_balance_reload: Arc::new(Mutex::new(Instant::now())),
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
            let assets = self.asset_store.get_assets_for_chain(chain.chain_id).await;

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
                match client
                    .get_token_accounts_by_owner(
                        &address,
                        TokenAccountsFilter::ProgramId(token_program),
                    )
                    .await
                {
                    Ok(response) => {
                        for keyed_account in response {
                            let data = match keyed_account.account.data {
                                UiAccountData::Json(data) => data,
                                _ => continue,
                            };

                            let Some((mint, amount)) = data.parsed.get("info").and_then(|info| {
                                let mint = Pubkey::from_str(info.get("mint")?.as_str()?).ok()?;
                                let amount: u128 = info
                                    .get("tokenAmount")?
                                    .get("amount")?
                                    .as_str()?
                                    .parse()
                                    .ok()?;
                                Some((mint, amount))
                            }) else {
                                continue;
                            };

                            if amount > 0 {
                                if let Some(asset) = mint_to_asset.get(&mint) {
                                    self.balances.lock().await.insert(asset.clone(), amount);
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
            let sol_balance_result = client.get_balance(&address).await;

            match sol_balance_result {
                Ok(lamports) => {
                    self.balances
                        .lock()
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
            let assets = self.asset_store.get_assets_for_chain(chain.chain_id).await;

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
                self.balances.lock().await.insert(asset.clone(), *amount);
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
                        .lock()
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
        let balances = self.balances.lock().await;

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
            "address" => %self.provider_manager.evm_address,
            "pubkey" => %self.provider_manager.svm_address
        );
    }

    async fn create_inventory_update_event(&self) -> SolverEvent {
        let balances = self.balances.lock().await.clone();
        SolverEvent::InventoryUpdate(InventoryUpdateEvent::new(balances))
    }

    async fn maybe_reload_balances(&self) -> Result<Option<SolverEvent>> {
        let should_reload = {
            let last_reload = self.last_balance_reload.lock().await;
            last_reload.elapsed() >= BALANCE_RELOAD_INTERVAL
        };

        if should_reload {
            info!(self.logger, "Periodic balance reload triggered");
            self.load_svm_balances().await?;
            self.load_evm_balances().await?;
            self.log_balances().await;
            *self.last_balance_reload.lock().await = Instant::now();
            return Ok(Some(self.create_inventory_update_event().await));
        }

        Ok(None)
    }
}

#[async_trait]
impl EventHandler for InventoryManager {
    fn name(&self) -> &'static str {
        "InventoryManager"
    }

    async fn initialize(&self) -> Result<()> {
        self.asset_store.initialize().await?;
        self.order_store.initialize().await?;

        // Load balances before starting
        self.load_svm_balances().await?;
        self.load_evm_balances().await?;
        self.log_balances().await;

        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let _ = self.order_store.handle_event(event.clone()).await;

        match event {
            SolverEvent::Start => {
                // Publish initial balances after initialization
                return Ok(vec![self.create_inventory_update_event().await]);
            }
            SolverEvent::RequestHold(e) => {
                let mut holds = self.holds.lock().await;
                let mut order_holds = self.order_holds.lock().await;
                let balance = self.balances.lock().await;

                let active_holds = holds
                    .get(&e.asset.chain)
                    .and_then(|h| h.get(&e.asset.address).cloned())
                    .unwrap_or(0);

                let raw_balance = balance.get(&e.asset).cloned().unwrap_or(0);
                let available = raw_balance.saturating_sub(active_holds);

                debug!(
                    self.logger,
                    "Hold request evaluation";
                    "order_id" => %e.order_id,
                    "asset" => %e.asset.symbol,
                    "asset_chain" => ?e.asset.chain,
                    "asset_address" => %e.asset.address,
                    "requested" => e.amount,
                    "raw_balance" => raw_balance,
                    "active_holds" => active_holds,
                    "available" => available,
                    "balance_keys" => ?balance.keys().map(|a| (&a.symbol, &a.chain, &a.address)).collect::<Vec<_>>(),
                );

                if available >= e.amount {
                    holds
                        .entry(e.asset.chain)
                        .or_insert_with(HashMap::new)
                        .insert(e.asset.address.clone(), active_holds + e.amount);

                    *order_holds.entry(e.order_id.clone()).or_insert(0) += e.amount;

                    return Ok(vec![SolverEvent::HoldSuccessful(HoldSuccessfulEvent::new(
                        e.order_id, e.amount,
                    ))]);
                } else {
                    let mut events = vec![];

                    if e.allow_partial_hold {
                        // Hold remaining amount and partially fill order
                        holds
                            .entry(e.asset.chain)
                            .or_insert_with(HashMap::new)
                            .insert(e.asset.address.clone(), active_holds + available);

                        *order_holds.entry(e.order_id.clone()).or_insert(0) += available;

                        events.push(SolverEvent::HoldSuccessful(HoldSuccessfulEvent::new(
                            e.order_id.clone(),
                            available,
                        )));
                    }

                    if !self.auto_rebalance {
                        warn!(
                            self.logger,
                            "Insufficient inventory for hold and auto-rebalance is disabled";
                            "order_id" => %e.order_id,
                            "asset" => &e.asset.symbol,
                            "requested_amount" => e.amount,
                            "available_amount" => available,
                        );

                        return Ok(events);
                    }

                    // naively acquire inventory with 5% buffer
                    let swap_amount = (e.amount - available) * 105 / 100;

                    let largest_balance = balance
                        .iter()
                        .filter(|(asset, _)| asset.chain == e.asset.chain && **asset != e.asset)
                        .max_by_key(|(_, balance)| *balance);

                    if let Some((token, &balance)) = largest_balance {
                        events.push(SolverEvent::RequestSwap(RequestSwapEvent::new(
                            e.order_id.clone(),
                            token.clone(),
                            e.asset,
                            min(balance, swap_amount),
                        )));
                    } else {
                        warn!(
                            self.logger,
                            "No suitable token found for rebalancing";
                            "chain" => %e.asset.chain,
                            "order_id" => %e.order_id
                        );
                    }

                    return Ok(events);
                }
            }
            SolverEvent::OrderCompleted(_) | SolverEvent::OrderCancelled(_) => {
                let order_id = event.order_id().unwrap();

                // Amount of funds held for the order
                let mut order_holds = self.order_holds.lock().await;
                let held = order_holds.remove(&order_id).unwrap_or(0);

                if held > 0 {
                    let order = self.order_store.get_order(&order_id).await?;
                    let token = format_address(&order.data.token_out);

                    // Release holds associated with the order
                    let mut holds = self.holds.lock().await;
                    if let Some(chain_holds) =
                        holds.get_mut(&chain_from_id(order.data.dest_chain_id))
                    {
                        if let Some(amount) = chain_holds.get_mut(&token) {
                            *amount = amount.saturating_sub(held);
                        }
                    }
                }
            }
            SolverEvent::FillOrderSuccessful(e) => {
                let order = self.order_store.get_order(&e.order_id).await?;

                let result = match chain_runtime(order.data.dest_chain_id) {
                    ChainRuntime::Evm => self.load_evm_balances().await,
                    ChainRuntime::Svm => self.load_svm_balances().await,
                };

                if let Err(e) = result {
                    error!(
                        self.logger,
                        "Failed to reload balances";
                        "chain_id" => %order.data.dest_chain_id,
                        "error" => %e,
                    );
                }

                // Publish updated balances after reload
                return Ok(vec![self.create_inventory_update_event().await]);
            }
            _ => {}
        }

        // Check for periodic balance reload
        if let Some(event) = self.maybe_reload_balances().await? {
            return Ok(vec![event]);
        }

        Ok(vec![])
    }
}
