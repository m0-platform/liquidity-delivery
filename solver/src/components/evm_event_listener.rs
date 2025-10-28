use alloy::primitives::{Address, Log, LogData};
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use async_trait::async_trait;
use futures_util::StreamExt;
use m0_liquidity_sdk::types::ChainRuntime;
use order_book::OrderData;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::config::ChainConfig;
use crate::error::{Result, SolverError};
use crate::events::{
    CancelRequest, EventBus, EventHandler, EventProcessor, Fill, OrderCancelRequestEvent,
    OrderCompleted, OrderCompletedEvent, OrderCreatedEvent, OrderFillEvent, OrderOpen,
    OrderRefundClaimedEvent, RefundClaimed, SolverEvent,
};
use crate::stores::OrderStore;
use crate::utils::{chain_runtime, decode_evm_address};

/// Component that listens to new orders created on multiple EVM chains
pub struct EvmEventListener {
    event_bus: Arc<EventBus>,
    order_store: Arc<RwLock<OrderStore>>,
    chains: Vec<ChainConfig>,
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
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
                        tracing::error!(
                            chain_id = ?chain.chain_id,
                            error = ?e,
                            "Failed to start event listener",
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
    pub fn new(event_bus: Arc<EventBus>, chains: Vec<ChainConfig>) -> Self {
        Self {
            task_handles: Arc::new(RwLock::new(Vec::new())),
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            chains,
            event_bus,
        }
    }

    async fn start_event_listener(&self, chain: &ChainConfig) -> Result<()> {
        let chain_id = chain.chain_id;

        if chain_runtime(chain_id) != ChainRuntime::Evm {
            return Ok(());
        }

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
                OrderOpen::SIGNATURE_HASH,
                Fill::SIGNATURE_HASH,
                CancelRequest::SIGNATURE_HASH,
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

        tracing::info!(
            "Started event listener for chain {} at {}",
            chain_id,
            chain.ws_url
        );

        let mut stream = sub.into_stream();
        let event_bus = self.event_bus.clone();

        // Keep the provider alive by moving it into the task
        let handle = tokio::spawn(async move {
            let _provider = provider; // Keep provider alive
            loop {
                if let Some(log) = stream.next().await {
                    match Self::process_log(chain_id, &log) {
                        Ok(Some(event)) => {
                            if let Err(e) = event_bus.publish(event).await {
                                tracing::error!(
                                    "Failed to publish event on chain {}: {}",
                                    chain_id,
                                    e
                                );
                            }
                        }
                        Ok(None) => (),
                        Err(e) => {
                            tracing::error!("Error processing log on chain {}: {}", chain_id, e);
                        }
                    }
                }
            }
        });

        // Store the task handle so we can abort it later
        let task_handles = self.task_handles.clone();
        tokio::spawn(async move {
            let mut handles = task_handles.write().await;
            handles.push(handle);
        });

        Ok(())
    }

    fn process_log(chain_id: u32, log: &alloy::rpc::types::Log) -> Result<Option<SolverEvent>> {
        let topics = &log.topics();

        if topics.is_empty() {
            return Ok(None);
        }

        let event_signature = topics[0];

        // Convert alloy::rpc::types::Log to Log for decoding
        let log_data = Log {
            address: Address::from_slice(log.address().as_slice()),
            data: LogData::new(log.topics().to_vec(), log.data().data.clone())
                .ok_or_else(|| SolverError::Component("Invalid log data".to_string()))?,
        };

        // Match on event signature and decode
        if event_signature == OrderOpen::SIGNATURE_HASH {
            return Ok(Some(Self::handle_order_open(chain_id, &log_data)?));
        } else if event_signature == Fill::SIGNATURE_HASH {
            return Ok(Some(Self::handle_fill(&log_data)?));
        } else if event_signature == CancelRequest::SIGNATURE_HASH {
            return Ok(Some(Self::handle_cancel_request(&log_data)?));
        } else if event_signature == RefundClaimed::SIGNATURE_HASH {
            return Ok(Some(Self::handle_refund_claimed(&log_data)?));
        } else if event_signature == OrderCompleted::SIGNATURE_HASH {
            return Ok(Some(Self::handle_order_completed(&log_data)?));
        }

        Ok(None)
    }

    fn handle_order_open(chain_id: u32, log: &Log) -> Result<SolverEvent> {
        let event = OrderOpen::decode_log(log)
            .map_err(|e| SolverError::Component(format!("Failed to decode OrderOpen: {}", e)))?;

        let order = OrderData {
            version: 0, // TODO: Get from contract or config
            origin_chain_id: chain_id,
            sender: [0u8; 32], // TODO: Extract from event
            nonce: 0,          // TODO: Extract from event
            dest_chain_id: event.destChainId,
            fill_deadline: 0, // TODO: Extract from event
            token_out: event.tokenOut.into(),
            recipient: event.solver.into(),
            amount_out: event.amountOut,
            solver: event.solver.into(),
        };

        Ok(SolverEvent::OrderCreated(OrderCreatedEvent::new(
            order,
            decode_evm_address(event.tokenIn),
        )))
    }

    fn handle_fill(log: &Log) -> Result<SolverEvent> {
        let event = Fill::decode_log(log)
            .map_err(|e| SolverError::Component(format!("Failed to decode Fill: {}", e)))?;

        let order_id = format!("{:x}", event.orderId);
        let fill_event = OrderFillEvent::new(order_id, event.amountOutFilled);

        Ok(SolverEvent::OrderFill(fill_event))
    }

    fn handle_cancel_request(log: &Log) -> Result<SolverEvent> {
        let event = CancelRequest::decode_log(log).map_err(|e| {
            SolverError::Component(format!("Failed to decode CancelRequest: {}", e))
        })?;

        let order_id = format!("{:x}", event.orderId);
        let cancel_event =
            OrderCancelRequestEvent::new(order_id, event.newFillDeadline.to::<u64>());

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
