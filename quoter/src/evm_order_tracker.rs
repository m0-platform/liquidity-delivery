use alloy::primitives::{Address, Log, LogData};
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use futures_util::StreamExt;
use slog::{error, info, Logger};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::config::ChainConfig;
use crate::contracts::IOrderBook::OrderOpened;
use crate::order_store::{OrderStore, TrackedOrder};

pub struct EvmOrderTracker {
    order_store: Arc<OrderStore>,
    chains: Vec<ChainConfig>,
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
    logger: Logger,
}

impl EvmOrderTracker {
    pub fn new(order_store: Arc<OrderStore>, chains: Vec<ChainConfig>, logger: Logger) -> Self {
        Self {
            order_store,
            chains,
            task_handles: Arc::new(RwLock::new(Vec::new())),
            logger: logger.new(slog::o!("component" => "EvmOrderTracker")),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for chain in &self.chains {
            if let Err(e) = self.start_chain_listener(chain).await {
                error!(self.logger, "Failed to start listener for chain";
                    "chain_id" => chain.chain_id,
                    "error" => %e
                );
            }
        }
        Ok(())
    }

    async fn start_chain_listener(
        &self,
        chain: &ChainConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let chain_id = chain.chain_id;
        let contract_address = Address::from_str(&chain.order_book_address)?;

        info!(self.logger, "Starting order tracker for chain"; "chain_id" => chain_id);

        // Fetch all historical events first
        self.fetch_historical_orders(chain, contract_address)
            .await?;

        // Start WebSocket subscription for new events
        let ws_handle = self
            .start_ws_subscription(chain.clone(), contract_address)
            .await?;

        let mut handles = self.task_handles.write().await;
        handles.push(ws_handle);

        info!(self.logger, "Started order tracker for chain";
            "chain_id" => chain_id,
            "ws_url" => %chain.ws_url
        );

        Ok(())
    }

    async fn fetch_historical_orders(
        &self,
        chain: &ChainConfig,
        contract_address: Address,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let rpc_url = chain.rpc_url.parse()?;
        let provider = ProviderBuilder::new().connect_http(rpc_url);
        let chain_id = chain.chain_id;
        let starting_block = chain.starting_block;

        let current_block = provider.get_block_number().await?;

        info!(self.logger, "Fetching historical orders";
            "chain_id" => chain_id,
            "from_block" => starting_block,
            "to_block" => current_block
        );

        // Fetch all OrderOpened events from starting_block
        let filter = Filter::new()
            .address(contract_address)
            .from_block(starting_block)
            .to_block(current_block)
            .event_signature(vec![OrderOpened::SIGNATURE_HASH]);

        let logs = provider.get_logs(&filter).await?;

        info!(self.logger, "Fetched historical orders";
            "count" => logs.len(),
            "chain_id" => chain_id
        );

        for log in logs {
            if let Err(e) = self.process_order_opened_log(chain_id, &log).await {
                error!(self.logger, "Error processing historical log"; "error" => %e);
            }
        }

        Ok(())
    }

    async fn start_ws_subscription(
        &self,
        chain: ChainConfig,
        contract_address: Address,
    ) -> Result<JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
        let ws = WsConnect::new(chain.ws_url.clone());
        let provider = ProviderBuilder::new().connect_ws(ws).await?;

        let filter = Filter::new()
            .address(contract_address)
            .event_signature(vec![OrderOpened::SIGNATURE_HASH]);

        let sub = provider.subscribe_logs(&filter).await?;
        let mut stream = sub.into_stream();

        let order_store = self.order_store.clone();
        let logger = self.logger.clone();
        let chain_id = chain.chain_id;

        let handle = tokio::spawn(async move {
            let _provider = provider; // Keep alive
            loop {
                if let Some(log) = stream.next().await {
                    if let Err(e) =
                        Self::process_order_opened_log_static(chain_id, &log, &order_store).await
                    {
                        error!(logger, "Error processing log"; "error" => %e);
                    }
                }
            }
        });

        Ok(handle)
    }

    async fn process_order_opened_log(
        &self,
        chain_id: u32,
        log: &alloy::rpc::types::Log,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Self::process_order_opened_log_static(chain_id, log, &self.order_store).await
    }

    async fn process_order_opened_log_static(
        chain_id: u32,
        log: &alloy::rpc::types::Log,
        order_store: &Arc<OrderStore>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let topics = log.topics();
        if topics.is_empty() {
            return Ok(());
        }

        let log_data = Log {
            address: Address::from_slice(log.address().as_slice()),
            data: LogData::new(topics.to_vec(), log.data().data.clone())
                .ok_or("Invalid log data")?,
        };

        let event = OrderOpened::decode_log(&log_data)?;
        let order_id = format!("{:x}", event.orderId);

        // Skip if already tracked
        if order_store.has_order(&order_id).await {
            return Ok(());
        }

        let tracked = TrackedOrder {
            order_id,
            origin_chain_id: chain_id,
            sender: format!("{:?}", event.sender),
            token_in: format!("{:?}", event.tokenIn),
            amount_in: event.amountIn.to_string(),
            dest_chain_id: event.destChainId,
            token_out: format!("{:x}", event.tokenOut),
            amount_out: event.amountOut.to_string(),
            solver: format!("{:x}", event.solver),
        };

        order_store.insert_order(tracked).await;
        Ok(())
    }
}
