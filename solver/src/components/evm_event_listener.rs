use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use async_trait::async_trait;
use futures_util::StreamExt;
use order_book::OrderData;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

use crate::config::ChainConfig;
use crate::error::{Result, SolverError};
use crate::events::{
    CancelRequest, EventBus, EventHandler, Fill, OrderCancelRequestEvent, OrderCompleted,
    OrderCompletedEvent, OrderCreatedEvent, OrderFillEvent, OrderOpen, OrderRefundClaimedEvent,
    RefundClaimed, SolverEvent,
};
use crate::stores::OrderStore;

/// Component that listens to new orders created on multiple EVM chains
pub struct EvmEventListener {
    event_bus: Arc<EventBus>,
    order_store: Arc<RwLock<OrderStore>>,
    chains: Vec<ChainConfig>,
}

impl EvmEventListener {
    pub fn new(event_bus: Arc<EventBus>, chains: Vec<ChainConfig>) -> Self {
        Self {
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            chains,
            event_bus,
        }
    }

    /// Start listening to events on a single chain
    async fn listen_to_chain(&self, chain: ChainConfig) -> Result<()> {
        let chain_id = chain.chain_id;
        tracing::info!(
            "Starting listener for chain {} at {}",
            chain_id,
            chain.order_book_address
        );

        // Use WebSocket if available, otherwise fall back to HTTP polling
        let ws_url = chain.ws_url.as_ref().unwrap_or(&chain.rpc_url);

        // Parse the OrderBook contract address
        let contract_address = Address::from_str(&chain.order_book_address)
            .map_err(|e| SolverError::Component(format!("Invalid contract address: {}", e)))?;

        // Create WebSocket connection
        let ws = WsConnect::new(ws_url);
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

        let mut stream = sub.into_stream();

        tokio::spawn(async move {
            loop {
                if let Some(log) = stream.next().await {
                    if let Err(e) = self.process_log(chain_id, &log).await {
                        tracing::error!("Error processing log on chain {}: {}", chain_id, e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Process a single log entry and publish corresponding event
    async fn process_log(&self, chain_id: u32, log: &alloy::rpc::types::Log) -> Result<()> {
        let topics = &log.topics();

        if topics.is_empty() {
            return Ok(());
        }

        let event_signature = topics[0];

        // Convert alloy::rpc::types::Log to alloy::primitives::Log for decoding
        let log_data = alloy::primitives::Log {
            address: Address::from_slice(log.address().as_slice()),
            data: alloy::primitives::LogData::new(log.topics().to_vec(), log.data().data.clone())
                .ok_or_else(|| SolverError::Component("Invalid log data".to_string()))?,
        };

        // Match on event signature and decode
        if event_signature == OrderOpen::SIGNATURE_HASH {
            self.handle_order_open(chain_id, &log_data).await?;
        } else if event_signature == Fill::SIGNATURE_HASH {
            self.handle_fill(&log_data).await?;
        } else if event_signature == CancelRequest::SIGNATURE_HASH {
            self.handle_cancel_request(&log_data).await?;
        } else if event_signature == RefundClaimed::SIGNATURE_HASH {
            self.handle_refund_claimed(&log_data).await?;
        } else if event_signature == OrderCompleted::SIGNATURE_HASH {
            self.handle_order_completed(&log_data).await?;
        }

        Ok(())
    }

    /// Handle OrderOpen event
    async fn handle_order_open(
        chain_id: u32,
        log: &alloy::primitives::Log,
        event_bus: &Arc<EventBus>,
    ) -> Result<()> {
        let event = OrderOpen::decode_log(log)
            .map_err(|e| SolverError::Component(format!("Failed to decode OrderOpen: {}", e)))?;

        tracing::info!(
            "OrderOpen event on chain {}: orderId={:?}",
            chain_id,
            event.orderId
        );

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

        let order_event = OrderCreatedEvent::new(order);

        event_bus
            .publish(Arc::new(SolverEvent::Created(order_event)))
            .await
            .map_err(|e| SolverError::EventBus(e.to_string()))?;

        Ok(())
    }

    /// Handle Fill event
    async fn handle_fill(log: &alloy::primitives::Log, event_bus: &Arc<EventBus>) -> Result<()> {
        let event = Fill::decode_log(log)
            .map_err(|e| SolverError::Component(format!("Failed to decode Fill: {}", e)))?;

        tracing::info!(
            "Fill event: orderId={:?}, amountOutFilled={}",
            event.orderId,
            event.amountOutFilled
        );

        let order_id = format!("{:x}", event.orderId);
        let fill_event = OrderFillEvent::new(order_id, event.amountOutFilled);

        event_bus
            .publish(Arc::new(SolverEvent::Fill(fill_event)))
            .await
            .map_err(|e| SolverError::EventBus(e.to_string()))?;

        Ok(())
    }

    /// Handle CancelRequest event
    async fn handle_cancel_request(
        log: &alloy::primitives::Log,
        event_bus: &Arc<EventBus>,
    ) -> Result<()> {
        let event = CancelRequest::decode_log(log).map_err(|e| {
            SolverError::Component(format!("Failed to decode CancelRequest: {}", e))
        })?;

        tracing::info!(
            "CancelRequest event: orderId={:?}, newFillDeadline={}",
            event.orderId,
            event.newFillDeadline
        );

        let order_id = format!("{:x}", event.orderId);
        let cancel_event =
            OrderCancelRequestEvent::new(order_id, event.newFillDeadline.to::<u64>());

        event_bus
            .publish(Arc::new(SolverEvent::CancelRequest(cancel_event)))
            .await
            .map_err(|e| SolverError::EventBus(e.to_string()))?;

        Ok(())
    }

    /// Handle RefundClaimed event
    async fn handle_refund_claimed(
        log: &alloy::primitives::Log,
        event_bus: &Arc<EventBus>,
    ) -> Result<()> {
        let event = RefundClaimed::decode_log(log).map_err(|e| {
            SolverError::Component(format!("Failed to decode RefundClaimed: {}", e))
        })?;

        tracing::info!(
            "RefundClaimed event: orderId={:?}, sender={:?}, amountInRefunded={}",
            event.orderId,
            event.sender,
            event.amountInRefunded
        );

        let order_id = format!("{:x}", event.orderId);
        let sender = format!("{:?}", event.sender);
        let refund_event = OrderRefundClaimedEvent::new(order_id, sender, event.amountInRefunded);

        event_bus
            .publish(Arc::new(SolverEvent::RefundClaimed(refund_event)))
            .await
            .map_err(|e| SolverError::EventBus(e.to_string()))?;

        Ok(())
    }

    /// Handle OrderCompleted event
    async fn handle_order_completed(
        log: &alloy::primitives::Log,
        event_bus: &Arc<EventBus>,
    ) -> Result<()> {
        let event = OrderCompleted::decode_log(log).map_err(|e| {
            SolverError::Component(format!("Failed to decode OrderCompleted: {}", e))
        })?;

        tracing::info!("OrderCompleted event: orderId={:?}", event.orderId);

        let order_id = format!("{:x}", event.orderId);
        let completed_event = OrderCompletedEvent::new(order_id);

        event_bus
            .publish(Arc::new(SolverEvent::Completed(completed_event)))
            .await
            .map_err(|e| SolverError::EventBus(e.to_string()))?;

        Ok(())
    }

    async fn start_event_listeners(&self) -> Result<()> {
        for chain in self.chains.clone() {
            tokio::spawn(async move {
                if let Err(e) = self::listen_to_chain(chain.clone(), chain_event_bus).await {
                    tracing::error!(
                        "Failed to start listener for chain {}: {}",
                        chain.chain_id,
                        e
                    );
                }
            });
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandler for EvmEventListener {
    fn name(&self) -> &'static str {
        "EvmEventListener"
    }

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn handle_event(&self, event: Arc<SolverEvent>) -> Result<Arc<Vec<SolverEvent>>> {
        let store = self.order_store.read().await;
        store.handle_event(event.clone()).await;

        match event.as_ref() {
            SolverEvent::Start => {
                self.start_event_listeners();
            }
            _ => {}
        }

        Ok(Arc::new(vec![]))
    }
}
