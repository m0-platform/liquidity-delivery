use anchor_client::{
    anchor_lang::{AnchorDeserialize, Discriminator},
    solana_sdk::{bs58, commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Signature},
};
use async_trait::async_trait;
use futures_util::StreamExt;
use m0_liquidity_sdk::types::ChainRuntime;
use order_book::{
    FillReported, NativeOrder, Order, OrderCancelled, OrderCompleted, OrderData, OrderFilled,
    OrderOpened, RefundClaimed,
};
use slog::{error, info, warn, Logger};
use solana_client::rpc_config::{
    RpcTransactionConfig, RpcTransactionLogsConfig, RpcTransactionLogsFilter,
};
use solana_transaction_status_client_types::{
    option_serializer, EncodedConfirmedTransactionWithStatusMeta, UiInstruction,
    UiTransactionEncoding,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::config::ChainConfig;
use crate::error::Result;
use crate::events::{
    EventBus, EventHandler, EventProcessor, OrderCancelledEvent, OrderCompletedEvent,
    OrderCreatedEvent, OrderFillEvent, OrderRefundClaimedEvent, SolverEvent,
};
use crate::providers::ProviderManager;
use crate::stores::OrderStore;
use crate::utils::chain_runtime;
use crate::{components::ComponentParams, utils::unix_timestamp_secs};

/// Enum representing all events emitted by the order_book program
enum OrderBookEvent {
    OrderOpened(OrderOpened),
    OrderFilled(OrderFilled),
    OrderCompleted(OrderCompleted),
    OrderCancelled(OrderCancelled),
    FillReported(FillReported),
    RefundClaimed(RefundClaimed),
}

pub struct SvmEventListener {
    event_bus: Arc<EventBus>,
    order_store: Arc<OrderStore>,
    chains: Vec<ChainConfig>,
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
    logger: Logger,
    provider_manager: Arc<ProviderManager>,
}

impl SvmEventListener {
    pub fn new(params: &ComponentParams) -> Self {
        let logger = params
            .logger
            .new(slog::o!("component" => "SvmEventListener"));

        Self {
            task_handles: Arc::new(RwLock::new(Vec::new())),
            order_store: Arc::new(OrderStore::new()),
            chains: params.config.chains.clone(),
            event_bus: params.event_bus.clone(),
            logger,
            provider_manager: params.provider_manager.clone(),
        }
    }
}

#[async_trait]
impl EventHandler for SvmEventListener {
    fn name(&self) -> &'static str {
        "SvmEventListener"
    }

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let _ = self.order_store.handle_event(event.clone()).await;

        match event {
            SolverEvent::Start => {
                for chain in self.chains.iter() {
                    self.start_event_listener(chain);
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

impl SvmEventListener {
    /// Parse all order_book events from transaction inner instructions
    fn parse_order_book_events(
        tx: &EncodedConfirmedTransactionWithStatusMeta,
        logger: &Logger,
        signature: &str,
    ) -> Vec<OrderBookEvent> {
        // Anchor event CPI discriminator (used for emit_cpi!)
        let anchor_event_discriminator: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];

        let mut events = Vec::new();

        let meta = match tx.transaction.meta.as_ref() {
            Some(meta) => meta,
            None => return events,
        };

        let inner = match &meta.inner_instructions {
            option_serializer::OptionSerializer::Some(inner) => inner,
            _ => return events,
        };

        for ui_inner in inner {
            for instruction in &ui_inner.instructions {
                if let UiInstruction::Compiled(compiled) = instruction {
                    let decoded_data = match bs58::decode(&compiled.data).into_vec() {
                        Ok(data) => data,
                        Err(_) => continue,
                    };

                    // Check for Anchor event CPI discriminator
                    if decoded_data.len() < 16 || decoded_data[0..8] != anchor_event_discriminator {
                        continue;
                    }

                    let event_discriminator = &decoded_data[8..16];
                    let mut data_slice = &decoded_data[16..];

                    // Try to parse each event type based on discriminator
                    if event_discriminator == OrderOpened::DISCRIMINATOR {
                        match OrderOpened::deserialize(&mut data_slice) {
                            Ok(event) => events.push(OrderBookEvent::OrderOpened(event)),
                            Err(e) => {
                                error!(
                                    logger,
                                    "Failed to deserialize OrderOpened event";
                                    "signature" => signature,
                                    "error" => %e,
                                );
                            }
                        }
                    } else if event_discriminator == OrderFilled::DISCRIMINATOR {
                        match OrderFilled::deserialize(&mut data_slice) {
                            Ok(event) => events.push(OrderBookEvent::OrderFilled(event)),
                            Err(e) => {
                                error!(
                                    logger,
                                    "Failed to deserialize OrderFilled event";
                                    "signature" => signature,
                                    "error" => %e,
                                );
                            }
                        }
                    } else if event_discriminator == OrderCompleted::DISCRIMINATOR {
                        match OrderCompleted::deserialize(&mut data_slice) {
                            Ok(event) => events.push(OrderBookEvent::OrderCompleted(event)),
                            Err(e) => {
                                error!(
                                    logger,
                                    "Failed to deserialize OrderCompleted event";
                                    "signature" => signature,
                                    "error" => %e,
                                );
                            }
                        }
                    } else if event_discriminator == OrderCancelled::DISCRIMINATOR {
                        match OrderCancelled::deserialize(&mut data_slice) {
                            Ok(event) => events.push(OrderBookEvent::OrderCancelled(event)),
                            Err(e) => {
                                error!(
                                    logger,
                                    "Failed to deserialize OrderCancelled event";
                                    "signature" => signature,
                                    "error" => %e,
                                );
                            }
                        }
                    } else if event_discriminator == FillReported::DISCRIMINATOR {
                        match FillReported::deserialize(&mut data_slice) {
                            Ok(event) => events.push(OrderBookEvent::FillReported(event)),
                            Err(e) => {
                                error!(
                                    logger,
                                    "Failed to deserialize FillReported event";
                                    "signature" => signature,
                                    "error" => %e,
                                );
                            }
                        }
                    } else if event_discriminator == RefundClaimed::DISCRIMINATOR {
                        match RefundClaimed::deserialize(&mut data_slice) {
                            Ok(event) => events.push(OrderBookEvent::RefundClaimed(event)),
                            Err(e) => {
                                error!(
                                    logger,
                                    "Failed to deserialize RefundClaimed event";
                                    "signature" => signature,
                                    "error" => %e,
                                );
                            }
                        }
                    }
                }
            }
        }

        events
    }

    fn start_event_listener(&self, chain: &ChainConfig) {
        if chain_runtime(chain.chain_id) != ChainRuntime::Svm {
            return;
        }

        info!(
            self.logger,
            "Starting event listener for chain";
            "chain_id" => %chain.chain_id,
        );

        let event_bus = self.event_bus.clone();
        let chain_id = chain.chain_id;
        let order_book_address = chain.order_book_address.clone();
        let logger = self.logger.clone();
        let providers = self.provider_manager.clone();

        let handle = tokio::spawn(async move {
            let provider = providers.get_svm_provider(chain_id).await.unwrap();
            let pubsub_client = provider.pubsub_client.clone();
            let program_id = Pubkey::from_str(&order_book_address).unwrap();

            let (mut log_stream, _unsub) = match pubsub_client
                .logs_subscribe(
                    RpcTransactionLogsFilter::Mentions(vec![program_id.to_string()]),
                    RpcTransactionLogsConfig {
                        commitment: Some(CommitmentConfig::confirmed()),
                    },
                )
                .await
            {
                Ok(subscription) => subscription,
                Err(e) => {
                    error!(
                        logger,
                        "Failed to subscribe to program logs";
                        "chain_id" => %chain_id,
                        "error" => %e,
                    );
                    return;
                }
            };

            while let Some(log_update) = log_stream.next().await {
                let signature_str = log_update.value.signature;

                let signature = Signature::from_str(&signature_str).unwrap();
                let rpc_client = provider.client().await;

                // Fetch the transaction with retry logic
                // The log subscription can fire before the transaction is fully indexed
                let mut tx = None;
                for attempt in 0..5 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    match rpc_client
                        .get_transaction_with_config(
                            &signature,
                            RpcTransactionConfig {
                                encoding: Some(UiTransactionEncoding::Json),
                                commitment: Some(CommitmentConfig::confirmed()),
                                max_supported_transaction_version: Some(0),
                            },
                        )
                        .await
                    {
                        Ok(transaction) => {
                            tx = Some(transaction);
                            break;
                        }
                        Err(e) => {
                            if attempt == 4 {
                                error!(
                                    logger,
                                    "Failed to fetch transaction after retries";
                                    "signature" => &signature_str,
                                    "chain_id" => %chain_id,
                                    "error" => %e,
                                );
                            }
                        }
                    }
                }

                let tx = match tx {
                    Some(t) => t,
                    None => continue,
                };

                // Parse all order_book events from transaction
                let events = Self::parse_order_book_events(&tx, &logger, &signature_str);

                if events.is_empty() {
                    warn!(
                        logger,
                        "No order_book events found in transaction";
                        "signature" => &signature_str,
                        "chain_id" => %chain_id,
                    );
                }

                for event in events {
                    let solver_event = match event {
                        OrderBookEvent::OrderOpened(e) => {
                            // Fetch data from order PDA for OrderOpened events
                            let (order_account, _) = Pubkey::find_program_address(
                                &[order_book::state::ORDER_SEED_PREFIX, &e.order_id[..]],
                                &program_id,
                            );

                            let mut order_result = None;
                            for attempt in 0..5 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                                match rpc_client.get_account_data(&order_account).await.and_then(
                                    |data| {
                                        let mut slice = &data[8..];
                                        Order::<NativeOrder>::deserialize(&mut slice)
                                            .map_err(|e| e.into())
                                    },
                                ) {
                                    Ok(order) => {
                                        order_result = Some(order);
                                        break;
                                    }
                                    Err(err) => {
                                        error!(
                                            logger,
                                            "Failed to fetch or deserialize order data after retries";
                                            "order_id" => hex::encode(e.order_id),
                                            "attempt" => attempt + 1,
                                            "error" => %err,
                                        );
                                    }
                                }
                            }

                            let order = match order_result {
                                Some(order) => order,
                                None => continue,
                            };

                            SolverEvent::OrderCreated(OrderCreatedEvent::new(
                                OrderData::new_from_native_order(order.data, chain_id),
                                signature_str.clone(),
                                tx.block_time
                                    .map(|t| t as u64)
                                    .unwrap_or_else(unix_timestamp_secs),
                            ))
                        }
                        OrderBookEvent::OrderFilled(e) => {
                            SolverEvent::OrderFill(OrderFillEvent::new(
                                hex::encode(e.order_id),
                                e.amount_out_filled,
                                signature_str.clone(),
                            ))
                        }
                        OrderBookEvent::OrderCompleted(e) => {
                            SolverEvent::OrderCompleted(OrderCompletedEvent::new(
                                hex::encode(e.order_id),
                                signature_str.clone(),
                                chain_id,
                            ))
                        }
                        OrderBookEvent::OrderCancelled(e) => {
                            SolverEvent::OrderCancelled(OrderCancelledEvent::new(
                                hex::encode(e.order_id),
                                signature_str.clone(),
                            ))
                        }
                        OrderBookEvent::FillReported(e) => {
                            SolverEvent::OrderFill(OrderFillEvent::new(
                                hex::encode(e.order_id),
                                e.amount_out_filled,
                                signature_str.clone(),
                            ))
                        }
                        OrderBookEvent::RefundClaimed(e) => {
                            SolverEvent::OrderRefundClaimed(OrderRefundClaimedEvent::new(
                                hex::encode(e.order_id),
                                e.sender.to_string(),
                                e.amount as u128,
                                signature_str.clone(),
                            ))
                        }
                    };

                    if let Err(e) = event_bus.publish(solver_event).await {
                        error!(
                            logger,
                            "Failed to publish event";
                            "chain_id" => %chain_id,
                            "error" => %e,
                        );
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
    }
}
