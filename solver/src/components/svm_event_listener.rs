use anchor_client::{
    anchor_lang::{AnchorDeserialize, Discriminator},
    solana_sdk::{bs58, commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Signature},
};
use async_trait::async_trait;
use futures_util::StreamExt;
use m0_liquidity_sdk::types::ChainRuntime;
use order_book::{NativeOrder, Order, OrderData, OrderOpened};
use slog::{error, info, Logger};
use solana_client::rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use solana_transaction_status_client_types::{
    option_serializer, EncodedConfirmedTransactionWithStatusMeta, UiInstruction,
    UiTransactionEncoding,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::components::ComponentParams;
use crate::config::ChainConfig;
use crate::error::Result;
use crate::events::{EventBus, EventHandler, EventProcessor, OrderCreatedEvent, SolverEvent};
use crate::providers::ProviderManager;
use crate::stores::OrderStore;
use crate::utils::chain_runtime;

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
    /// Parse OrderOpened event from transaction inner instructions
    fn parse_order_opened_event(
        tx: &EncodedConfirmedTransactionWithStatusMeta,
        logger: &Logger,
        signature: &str,
    ) -> Option<OrderOpened> {
        let target_discriminator: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];
        let order_event_discriminator = OrderOpened::DISCRIMINATOR;

        let meta = tx.transaction.meta.as_ref()?;
        let inner = match &meta.inner_instructions {
            option_serializer::OptionSerializer::Some(inner) => inner,
            _ => return None,
        };

        for ui_inner in inner {
            for instruction in &ui_inner.instructions {
                if let UiInstruction::Compiled(compiled) = instruction {
                    let decoded_data = match bs58::decode(&compiled.data).into_vec() {
                        Ok(data) => data,
                        Err(_) => continue,
                    };

                    if decoded_data.len() < 16
                        || decoded_data[0..8] != target_discriminator
                        || &decoded_data[8..16] != order_event_discriminator
                    {
                        continue;
                    }

                    let mut data_slice = &decoded_data[16..];
                    match OrderOpened::deserialize(&mut data_slice) {
                        Ok(event) => {
                            return Some(event);
                        }
                        Err(e) => {
                            error!(
                                logger,
                                "Failed to deserialize OrderOpened event";
                                "signature" => signature,
                                "error" => %e,
                            );
                        }
                    }
                }
            }
        }

        None
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
                let logs = log_update.value.logs;

                // Check for open_order call in logs
                if !logs
                    .iter()
                    .any(|log| log.contains("Program log: Instruction: OpenOrder"))
                {
                    continue;
                }

                let signature = Signature::from_str(&signature_str).unwrap();
                let rpc_client = provider.client().await;

                // Fetch the transaction
                let tx = match rpc_client
                    .get_transaction(&signature, UiTransactionEncoding::Json)
                    .await
                {
                    Ok(transaction) => transaction,
                    Err(e) => {
                        error!(
                            logger,
                            "Failed to fetch transaction";
                            "signature" => &signature_str,
                            "chain_id" => %chain_id,
                            "error" => %e,
                        );
                        continue;
                    }
                };

                // Parse OrderOpened event from transaction
                let Some(event) = Self::parse_order_opened_event(&tx, &logger, &signature_str)
                else {
                    continue;
                };

                // Fetch data from order PDA
                let (order_account, _) = Pubkey::find_program_address(
                    &[order_book::state::ORDER_SEED_PREFIX, &event.order_id[..]],
                    &program_id,
                );

                let order =
                    match rpc_client
                        .get_account_data(&order_account)
                        .await
                        .and_then(|data| {
                            let mut slice = &data[8..];
                            Order::<NativeOrder>::deserialize(&mut slice).map_err(|e| e.into())
                        }) {
                        Ok(order) => order,
                        Err(e) => {
                            error!(
                                logger,
                                "Failed to fetch or deserialize order data";
                                "order_id" => hex::encode(event.order_id),
                                "error" => %e,
                            );
                            continue;
                        }
                    };

                let order_event =
                    OrderCreatedEvent::new(OrderData::new_from_native_order(order.data, chain_id));

                if let Err(e) = event_bus
                    .publish(SolverEvent::OrderCreated(order_event))
                    .await
                {
                    error!(
                        logger,
                        "Failed to publish OrderCreated event";
                        "chain_id" => %chain_id,
                        "error" => %e,
                    );
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
