use alloy::primitives::{Address, Log, LogData};
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use async_trait::async_trait;
use futures_util::StreamExt;
use m0_liquidity_sdk::types::ChainRuntime;
use order_book::OrderData;
use slog::{error, info, Logger};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::interval;

use crate::components::ComponentParams;
use crate::config::{ChainConfig, Network};
use crate::contracts::evm::IOrderBook;
use crate::error::{Result, SolverError};
use crate::events::{
    CancelRequested, EventBus, EventHandler, EventProcessor, OrderCancelRequestEvent,
    OrderCompleted, OrderCompletedEvent, OrderCreatedEvent, OrderFillEvent, OrderFilled,
    OrderOpened, OrderRefundClaimedEvent, RefundClaimed, SolverEvent,
};
use crate::stores::OrderStore;
use crate::utils::{chain_runtime, decode_evm_address};

/// Component that listens to new orders created on multiple EVM chains
pub struct EvmEventListener {
    event_bus: Arc<EventBus>,
    order_store: Arc<RwLock<OrderStore>>,
    chains: Vec<ChainConfig>,
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
    logger: Logger,
    seen_logs: Arc<RwLock<HashSet<(u32, u64, u64)>>>, // (chain_id, block_number, log_index)
    last_polled_block: Arc<RwLock<HashMap<u32, u64>>>,
    polling_interval: Duration,
}

#[async_trait]
impl EventHandler for EvmEventListener {
    fn name(&self) -> &'static str {
        "EvmEventListener"
    }

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let store = self.order_store.read().await;
        let _ = store.handle_event(event.clone()).await;

        match event {
            SolverEvent::Start => {
                for chain in self.chains.iter() {
                    if let Err(e) = self.start_event_listener(&chain).await {
                        error!(
                            self.logger,
                            "Failed to start event listener";
                            "chain_id" => ?chain.chain_id,
                            "error" => ?e,
                        );
                    }
                }
            }
            SolverEvent::Stop => {
                let mut handles = self.task_handles.write().await;
                for handle in handles.drain(..) {
                    handle.abort();
                }
            }
            _ => {}
        }

        Ok(vec![])
    }
}

impl EvmEventListener {
    pub fn new(params: &ComponentParams) -> Self {
        Self {
            task_handles: Arc::new(RwLock::new(Vec::new())),
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            chains: params.config.chains.clone(),
            event_bus: params.event_bus.clone(),
            logger: params
                .logger
                .new(slog::o!("component" => "EvmEventListener")),
            seen_logs: Arc::new(RwLock::new(HashSet::new())),
            last_polled_block: Arc::new(RwLock::new(HashMap::new())),
            polling_interval: if params.config.network == Network::Local {
                Duration::from_millis(1000)
            } else {
                Duration::from_millis(5000)
            },
        }
    }

    async fn start_event_listener(&self, chain: &ChainConfig) -> Result<()> {
        let chain_id = chain.chain_id;

        if chain_runtime(chain_id) != ChainRuntime::Evm {
            return Ok(());
        }

        info!(
            self.logger,
            "Starting event listener for chain";
            "chain_id" => %chain_id,
        );

        // Parse the OrderBook contract address
        let contract_address = Address::from_str(&chain.order_book_address)
            .map_err(|e| SolverError::Component(format!("Invalid contract address: {}", e)))?;

        // Create WebSocket connection
        let ws = WsConnect::new(chain.ws_url.clone());
        let provider = ProviderBuilder::new().connect_ws(ws).await.map_err(|e| {
            SolverError::Component(format!("Failed to connect to chain {}: {}", chain_id, e))
        })?;

        // Create filters for all events
        let filter = Filter::new()
            .address(contract_address)
            .event_signature(vec![
                OrderOpened::SIGNATURE_HASH,
                OrderFilled::SIGNATURE_HASH,
                CancelRequested::SIGNATURE_HASH,
                RefundClaimed::SIGNATURE_HASH,
                OrderCompleted::SIGNATURE_HASH,
            ]);

        // Subscribe to logs
        let sub = provider.subscribe_logs(&filter).await.map_err(|e| {
            SolverError::Component(format!(
                "Failed to subscribe to logs on chain {}: {}",
                chain_id, e
            ))
        })?;

        let mut stream = sub.into_stream();
        let event_bus = self.event_bus.clone();
        let logger = self.logger.clone();
        let provider_clone = provider.clone();

        // Keep the provider alive by moving it into the task
        let ws_handle = tokio::spawn(async move {
            let _provider = provider; // Keep provider alive
            loop {
                if let Some(log) = stream.next().await {
                    match Self::process_log(chain_id, &log, contract_address, &provider_clone).await
                    {
                        Ok(Some(event)) => {
                            if let Err(e) = event_bus.publish(event).await {
                                error!(
                                    logger,
                                    "Failed to publish event on chain";
                                    "chain_id" => %chain_id,
                                    "error" => %e,
                                );
                            }
                        }
                        Ok(None) => (),
                        Err(e) => {
                            error!(
                                logger,
                                "Error processing log on chain";
                                "chain_id" => %chain_id,
                                "error" => %e,
                            );
                        }
                    }
                }
            }
        });

        // Start polling task (will fetch historical logs on first poll)
        let poll_handle = self.start_polling_task(chain.clone()).await?;

        // Store task handles
        let task_handles = self.task_handles.clone();
        tokio::spawn(async move {
            let mut handles = task_handles.write().await;
            handles.push(ws_handle);
            handles.push(poll_handle);
        });

        info!(
            self.logger,
            "Started event listener for chain";
            "chain_id" => %chain_id,
            "ws_url" => %chain.ws_url,
        );

        Ok(())
    }

    async fn start_polling_task(&self, chain: ChainConfig) -> Result<JoinHandle<()>> {
        let chain_id = chain.chain_id;
        let contract_address = Address::from_str(&chain.order_book_address)
            .map_err(|e| SolverError::Component(format!("Invalid contract address: {}", e)))?;

        // Create HTTP provider for polling
        let rpc_url = chain.rpc_url.parse().unwrap();
        let provider = ProviderBuilder::new().connect_http(rpc_url);

        let event_bus = self.event_bus.clone();
        let seen_logs = self.seen_logs.clone();
        let last_polled_block = self.last_polled_block.clone();
        let polling_interval = self.polling_interval;
        let logger = self.logger.clone();

        let handle = tokio::spawn(async move {
            let mut poll_interval = interval(polling_interval);

            loop {
                poll_interval.tick().await;

                let from_block = {
                    let last_polled = last_polled_block.read().await;
                    last_polled.get(&chain_id).copied()
                };

                match provider.get_block_number().await {
                    Ok(to_block) => {
                        // First poll fetch last 1000 blocks
                        let from_block = from_block.unwrap_or(to_block.saturating_sub(1000));

                        if from_block >= to_block {
                            continue;
                        }

                        if let Err(e) = Self::fetch_historical_logs(
                            chain_id,
                            contract_address,
                            from_block + 1,
                            to_block,
                            &provider,
                            &event_bus,
                            &seen_logs,
                            &logger,
                        )
                        .await
                        {
                            error!(logger, "Failed to fetch logs for chain {}: {}", chain_id, e);
                        } else {
                            let mut last_polled = last_polled_block.write().await;
                            last_polled.insert(chain_id, to_block);
                        }
                    }
                    Err(e) => {
                        error!(
                            logger,
                            "Failed to get block number for chain {}: {}", chain_id, e
                        );
                    }
                }
            }
        });

        Ok(handle)
    }

    async fn fetch_historical_logs<P: Provider>(
        chain_id: u32,
        contract_address: Address,
        from_block: u64,
        to_block: u64,
        provider: &P,
        event_bus: &Arc<EventBus>,
        seen_logs: &Arc<RwLock<HashSet<(u32, u64, u64)>>>,
        logger: &Logger,
    ) -> Result<()> {
        let filter = Filter::new()
            .address(contract_address)
            .from_block(from_block)
            .to_block(to_block)
            .event_signature(vec![
                OrderOpened::SIGNATURE_HASH,
                OrderFilled::SIGNATURE_HASH,
                CancelRequested::SIGNATURE_HASH,
                RefundClaimed::SIGNATURE_HASH,
                OrderCompleted::SIGNATURE_HASH,
            ]);

        let logs = provider.get_logs(&filter).await.map_err(|e| {
            SolverError::Component(format!(
                "Failed to fetch historical logs for chain {}: {}",
                chain_id, e
            ))
        })?;

        info!(
            logger,
            "Fetched historical logs";
            "count" => logs.len(),
            "chain_id" => chain_id,
            "from_block" => from_block,
            "to_block" => to_block
        );

        for log in logs {
            let block_number = log.block_number.unwrap();
            let log_index = log.log_index.unwrap();

            // Check if log has been seen
            let seen = seen_logs.read().await;
            if seen.contains(&(chain_id, block_number, log_index as u64)) {
                continue;
            }

            // Mark as seen
            let mut seen = seen_logs.write().await;
            seen.insert((chain_id, block_number, log_index as u64));

            match Self::process_log(chain_id, &log, contract_address, provider).await {
                Ok(Some(event)) => {
                    if let Err(e) = event_bus.publish(event).await {
                        error!(
                            logger,
                            "Failed to publish historical event on chain {}: {}", chain_id, e
                        );
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    error!(
                        logger,
                        "Error processing historical log on chain {}: {}", chain_id, e
                    );
                }
            }
        }

        Ok(())
    }

    async fn process_log<P: Provider>(
        chain_id: u32,
        log: &alloy::rpc::types::Log,
        contract_address: Address,
        provider: &P,
    ) -> Result<Option<SolverEvent>> {
        let topics = &log.topics();

        if topics.is_empty() {
            return Ok(None);
        }

        let event_signature = topics[0];

        let log_data = Log {
            address: Address::from_slice(log.address().as_slice()),
            data: LogData::new(log.topics().to_vec(), log.data().data.clone())
                .ok_or_else(|| SolverError::Component("Invalid log data".to_string()))?,
        };

        // Match on event signature and decode
        if event_signature == OrderOpened::SIGNATURE_HASH {
            let event =
                Self::handle_order_open(chain_id, &log_data, contract_address, provider).await?;
            return Ok(Some(event));
        } else if event_signature == OrderFilled::SIGNATURE_HASH {
            return Ok(Some(Self::handle_fill(&log_data)?));
        } else if event_signature == CancelRequested::SIGNATURE_HASH {
            return Ok(Some(Self::handle_cancel_request(&log_data)?));
        } else if event_signature == RefundClaimed::SIGNATURE_HASH {
            return Ok(Some(Self::handle_refund_claimed(&log_data)?));
        } else if event_signature == OrderCompleted::SIGNATURE_HASH {
            return Ok(Some(Self::handle_order_completed(&log_data)?));
        }

        Ok(None)
    }

    async fn handle_order_open<P: Provider>(
        chain_id: u32,
        log: &Log,
        contract_address: Address,
        provider: &P,
    ) -> Result<SolverEvent> {
        let event = OrderOpened::decode_log(log)
            .map_err(|e| SolverError::Component(format!("Failed to decode OrderOpen: {}", e)))?;

        let order_id = event.orderId;

        // Create contract instance and call getOrder
        let contract = IOrderBook::new(contract_address, provider);
        let order_result = contract
            .getOrder(order_id)
            .call()
            .await
            .map_err(|e| SolverError::Component(format!("Failed to call getOrder: {}", e)))?;

        let order = OrderData {
            version: order_result.version,
            origin_chain_id: chain_id,
            sender: decode_evm_address(event.sender),
            nonce: order_result.nonce,
            dest_chain_id: event.destChainId,
            fill_deadline: order_result.fillDeadline as u64,
            token_in: decode_evm_address(event.tokenIn),
            token_out: event.tokenOut.into(),
            recipient: order_result.recipient.into(),
            amount_in: event.amountIn,
            amount_out: event.amountOut,
            solver: event.solver.into(),
        };

        Ok(SolverEvent::OrderCreated(OrderCreatedEvent::new(
            order,
            decode_evm_address(event.tokenIn),
        )))
    }

    fn handle_fill(log: &Log) -> Result<SolverEvent> {
        let event = OrderFilled::decode_log(log)
            .map_err(|e| SolverError::Component(format!("Failed to decode Fill: {}", e)))?;

        let order_id = format!("{:x}", event.orderId);
        let fill_event = OrderFillEvent::new(order_id, event.amountOutFilled);

        Ok(SolverEvent::OrderFill(fill_event))
    }

    fn handle_cancel_request(log: &Log) -> Result<SolverEvent> {
        let event = CancelRequested::decode_log(log).map_err(|e| {
            SolverError::Component(format!("Failed to decode CancelRequest: {}", e))
        })?;

        let order_id = format!("{:x}", event.orderId);
        let cancel_event = OrderCancelRequestEvent::new(order_id, event.cancelRequestedAt as u64);

        Ok(SolverEvent::OrderCancelRequest(cancel_event))
    }

    fn handle_refund_claimed(log: &Log) -> Result<SolverEvent> {
        let event = RefundClaimed::decode_log(log).map_err(|e| {
            SolverError::Component(format!("Failed to decode RefundClaimed: {}", e))
        })?;

        let order_id = format!("{:x}", event.orderId);
        let sender = format!("{:?}", event.sender);
        let refund_event = OrderRefundClaimedEvent::new(order_id, sender, event.amountInRefunded);

        Ok(SolverEvent::OrderRefundClaimed(refund_event))
    }

    fn handle_order_completed(log: &Log) -> Result<SolverEvent> {
        let event = OrderCompleted::decode_log(log).map_err(|e| {
            SolverError::Component(format!("Failed to decode OrderCompleted: {}", e))
        })?;

        let order_id = format!("{:x}", event.orderId);
        let completed_event = OrderCompletedEvent::new(order_id);

        Ok(SolverEvent::OrderCompleted(completed_event))
    }
}
