use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::sol_types::{sol, SolEvent};
use async_trait::async_trait;
use futures_util::StreamExt;
use order_book::OrderData;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

use crate::components::Component;
use crate::config::ChainConfig;
use crate::error::{Result, SolverError};
use crate::events::{
    EventHandler, OrderCancelRequestEvent, OrderCompletedEvent, OrderCreatedEvent, OrderFillEvent,
    OrderRefundClaimedEvent,
};
use crate::{EventBus, OrderEvent, OrderStore};

// Define Solidity events using alloy's sol! macro
sol! {
    #![sol(rpc, alloy_sol_types = alloy::sol_types)]

    #[derive(Debug)]
    event OrderOpen(
        bytes32 indexed orderId,
        address tokenIn,
        uint128 amountIn,
        uint32 indexed destChainId,
        bytes32 indexed tokenOut,
        uint128 amountOut,
        bytes32 solver
    );

    #[derive(Debug)]
    event Fill(
        bytes32 indexed orderId,
        address indexed solver,
        uint128 amountOutFilled
    );

    #[derive(Debug)]
    event CancelRequest(
        bytes32 indexed orderId,
        uint40 newFillDeadline
    );

    #[derive(Debug)]
    event RefundClaimed(
        bytes32 indexed orderId,
        address indexed sender,
        uint128 amountInRefunded
    );

    #[derive(Debug)]
    event OrderCompleted(
        bytes32 indexed orderId
    );
}

/// Component that listens to new orders created on multiple EVM chains
pub struct OrderListener {
    order_store: Arc<RwLock<OrderStore>>,
    chains: Vec<ChainConfig>,
}

impl OrderListener {
    pub fn new(chains: Vec<ChainConfig>) -> Self {
        Self {
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            chains,
        }
    }

    /// Start listening to events on a single chain
    async fn listen_to_chain(
        chain: ChainConfig,
        event_bus: Arc<EventBus>,
        mut shutdown_rx: Receiver<()>,
    ) -> Result<()> {
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
            .map_err(|e| SolverError::InvalidAddress(format!("Invalid contract address: {}", e)))?;

        // Create WebSocket connection
        let ws = WsConnect::new(ws_url);
        let provider = ProviderBuilder::new().connect_ws(ws).await.map_err(|e| {
            SolverError::Transport(format!("Failed to connect to chain {}: {}", chain_id, e))
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
            SolverError::Rpc(format!(
                "Failed to subscribe to logs on chain {}: {}",
                chain_id, e
            ))
        })?;

        let mut stream = sub.into_stream();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Chain {} listener received shutdown signal", chain_id);
                        break;
                    }
                    Some(log) = stream.next() => {
                        if let Err(e) = Self::process_log(chain_id, &log, &event_bus).await {
                            tracing::error!("Error processing log on chain {}: {}", chain_id, e);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Process a single log entry and publish corresponding event
    async fn process_log(
        chain_id: u32,
        log: &alloy::rpc::types::Log,
        event_bus: &Arc<EventBus>,
    ) -> Result<()> {
        let topics = &log.topics();

        if topics.is_empty() {
            return Ok(());
        }

        let event_signature = topics[0];

        // Convert alloy::rpc::types::Log to alloy::primitives::Log for decoding
        let log_data = alloy::primitives::Log {
            address: Address::from_slice(log.address().as_slice()),
            data: alloy::primitives::LogData::new(log.topics().to_vec(), log.data().data.clone())
                .ok_or_else(|| SolverError::EventParsing("Invalid log data".to_string()))?,
        };

        // Match on event signature and decode
        if event_signature == OrderOpen::SIGNATURE_HASH {
            Self::handle_order_open(chain_id, &log_data, event_bus).await?;
        } else if event_signature == Fill::SIGNATURE_HASH {
            Self::handle_fill(&log_data, event_bus).await?;
        } else if event_signature == CancelRequest::SIGNATURE_HASH {
            Self::handle_cancel_request(&log_data, event_bus).await?;
        } else if event_signature == RefundClaimed::SIGNATURE_HASH {
            Self::handle_refund_claimed(&log_data, event_bus).await?;
        } else if event_signature == OrderCompleted::SIGNATURE_HASH {
            Self::handle_order_completed(&log_data, event_bus).await?;
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
            .map_err(|e| SolverError::EventParsing(format!("Failed to decode OrderOpen: {}", e)))?;

        tracing::info!(
            "OrderOpen event on chain {}: orderId={:?}",
            chain_id,
            event.orderId
        );

        // Convert to internal OrderData structure
        let order = OrderData {
            version: 0, // TODO: Get from contract or config
            origin_chain_id: chain_id,
            sender: Self::address_to_bytes32(event.tokenIn),
            nonce: 0, // TODO: Extract from orderId or fetch from contract
            dest_chain_id: event.destChainId,
            fill_deadline: 0, // TODO: Extract from contract
            token_out: event.tokenOut.into(),
            recipient: event.solver.into(),
            amount_out: event.amountOut,
            solver: event.solver.into(),
        };

        let order_event = OrderCreatedEvent::new(order);

        event_bus
            .publish(Arc::new(OrderEvent::Created(order_event)))
            .await
            .map_err(|e| SolverError::EventBus(e.to_string()))?;

        Ok(())
    }

    /// Handle Fill event
    async fn handle_fill(log: &alloy::primitives::Log, event_bus: &Arc<EventBus>) -> Result<()> {
        let event = Fill::decode_log(log)
            .map_err(|e| SolverError::EventParsing(format!("Failed to decode Fill: {}", e)))?;

        tracing::info!(
            "Fill event: orderId={:?}, amountOutFilled={}",
            event.orderId,
            event.amountOutFilled
        );

        let order_id = format!("{:x}", event.orderId);
        let fill_event = OrderFillEvent::new(order_id, event.amountOutFilled as u64);

        event_bus
            .publish(Arc::new(OrderEvent::Fill(fill_event)))
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
            SolverError::EventParsing(format!("Failed to decode CancelRequest: {}", e))
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
            .publish(Arc::new(OrderEvent::CancelRequest(cancel_event)))
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
            SolverError::EventParsing(format!("Failed to decode RefundClaimed: {}", e))
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
            .publish(Arc::new(OrderEvent::RefundClaimed(refund_event)))
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
            SolverError::EventParsing(format!("Failed to decode OrderCompleted: {}", e))
        })?;

        tracing::info!("OrderCompleted event: orderId={:?}", event.orderId);

        let order_id = format!("{:x}", event.orderId);
        let completed_event = OrderCompletedEvent::new(order_id);

        event_bus
            .publish(Arc::new(OrderEvent::Completed(completed_event)))
            .await
            .map_err(|e| SolverError::EventBus(e.to_string()))?;

        Ok(())
    }

    /// Helper function to convert Address to [u8; 32]
    fn address_to_bytes32(addr: Address) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[12..32].copy_from_slice(addr.as_slice());
        bytes
    }
}

#[async_trait]
impl Component for OrderListener {
    fn name() -> &'static str {
        "OrderListener"
    }

    async fn initialize(&self) -> Result<()> {
        tracing::info!(
            "Initializing OrderListener for {} chains",
            self.chains.len()
        );
        Ok(())
    }

    async fn start(&self, event_bus: Arc<EventBus>, shutdown_rx: Receiver<()>) -> Result<()> {
        tracing::info!("Starting OrderListener");

        // Task to handle events
        let order_store = self.order_store.clone();
        Self::spawn_event_handler(event_bus.clone(), shutdown_rx.resubscribe(), move |event| {
            let store = order_store.clone();
            async move {
                let store = store.read().await;
                store.handle_event(event).await
            }
        });

        // Start a listener for each configured chain
        for chain in self.chains.clone() {
            let chain_event_bus = event_bus.clone();
            let chain_shutdown = shutdown_rx.resubscribe();

            tokio::spawn(async move {
                if let Err(e) =
                    Self::listen_to_chain(chain.clone(), chain_event_bus, chain_shutdown).await
                {
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
