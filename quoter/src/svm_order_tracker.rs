use borsh::BorshDeserialize;
use futures_util::StreamExt;
use slog::{error, info, Logger};
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::{
    option_serializer, EncodedConfirmedTransactionWithStatusMeta, UiInstruction,
    UiTransactionEncoding,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::config::{ChainConfig, ChainType};
use crate::order_store::{OrderStore, TrackedOrder};

/// Seed prefix for order PDAs (must match the SVM program)
const ORDER_SEED_PREFIX: &[u8] = b"order";

/// Anchor event discriminator prefix
const EVENT_DISCRIMINATOR: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];

/// OrderOpened event discriminator (from Anchor)
const ORDER_OPENED_DISCRIMINATOR: [u8; 8] = [66, 178, 244, 109, 33, 137, 241, 61];

/// SVM OrderStatus enum (must match the SVM program)
#[derive(BorshDeserialize, Debug, Clone, PartialEq)]
#[repr(u8)]
enum SvmOrderStatus {
    DoesNotExist,
    Created,
    CancelRequested,
    Completed,
}

/// SVM OrderType enum (must match the SVM program)
#[derive(BorshDeserialize, Debug, Clone)]
#[repr(u8)]
enum SvmOrderType {
    Native,
    Foreign,
}

/// SVM NativeOrder struct (must match the SVM program)
#[derive(BorshDeserialize, Debug)]
struct SvmNativeOrder {
    pub status: SvmOrderStatus,
    pub version: u16,
    pub sender: Pubkey,
    pub nonce: u64,
    pub dest_chain_id: u32,
    pub fill_deadline: u64,
    pub cancel_requested_at: u64,
    pub token_in: Pubkey,
    pub token_out: [u8; 32],
    pub amount_in: u128,
    pub amount_out: u128,
    pub recipient: [u8; 32],
    pub solver: [u8; 32],
    pub amount_in_released: u128,
    pub amount_out_filled: u128,
}

/// SVM Order wrapper struct (must match the SVM program)
#[derive(BorshDeserialize, Debug)]
struct SvmOrder {
    pub order_type: SvmOrderType,
    pub bump: u8,
    pub data: SvmNativeOrder,
}

/// OrderOpened event structure (must match the SVM program)
#[derive(BorshDeserialize, Debug)]
struct OrderOpenedEvent {
    pub order_id: [u8; 32],
    pub sender: Pubkey,
    pub token_in: Pubkey,
    pub amount_in: u128,
    pub dest_chain_id: u32,
    pub token_out: [u8; 32],
    pub amount_out: u128,
    pub solver: [u8; 32],
}

pub struct SvmOrderTracker {
    order_store: Arc<OrderStore>,
    chains: Vec<ChainConfig>,
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
    logger: Logger,
}

impl SvmOrderTracker {
    pub fn new(order_store: Arc<OrderStore>, chains: Vec<ChainConfig>, logger: Logger) -> Self {
        // Filter to only SVM chains
        let svm_chains: Vec<ChainConfig> = chains
            .into_iter()
            .filter(|c| c.chain_type == ChainType::Svm)
            .collect();

        Self {
            order_store,
            chains: svm_chains,
            task_handles: Arc::new(RwLock::new(Vec::new())),
            logger: logger.new(slog::o!("component" => "SvmOrderTracker")),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for chain in &self.chains {
            if let Err(e) = self.start_chain_listener(chain).await {
                error!(self.logger, "Failed to start SVM listener for chain";
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
        let program_id = Pubkey::from_str(&chain.order_book_address)?;

        info!(self.logger, "Starting SVM order tracker for chain"; "chain_id" => chain_id);

        // Start WebSocket subscription for new events
        let ws_handle = self.start_ws_subscription(chain.clone(), program_id).await?;

        let mut handles = self.task_handles.write().await;
        handles.push(ws_handle);

        info!(self.logger, "Started SVM order tracker for chain";
            "chain_id" => chain_id,
            "ws_url" => %chain.ws_url
        );

        Ok(())
    }

    async fn start_ws_subscription(
        &self,
        chain: ChainConfig,
        program_id: Pubkey,
    ) -> Result<JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
        let pubsub_client = PubsubClient::new(&chain.ws_url).await?;
        let order_store = self.order_store.clone();
        let logger = self.logger.clone();
        let chain_id = chain.chain_id;
        let rpc_url = chain.rpc_url.clone();

        let handle = tokio::spawn(async move {
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

            info!(logger, "Subscribed to SVM program logs"; "chain_id" => chain_id);

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

                info!(logger, "Detected OpenOrder transaction";
                    "signature" => &signature_str,
                    "chain_id" => chain_id
                );

                let signature = match Signature::from_str(&signature_str) {
                    Ok(sig) => sig,
                    Err(e) => {
                        error!(logger, "Failed to parse signature"; "error" => %e);
                        continue;
                    }
                };

                let rpc_client = RpcClient::new(rpc_url.clone());

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

                let order_id = format!("0x{}", hex::encode(event.order_id));

                // Skip if already tracked
                if order_store.has_order(&order_id).await {
                    continue;
                }

                // Fetch full order data from PDA
                let (order_pda, _) = Pubkey::find_program_address(
                    &[ORDER_SEED_PREFIX, &event.order_id[..]],
                    &program_id,
                );

                let order = match rpc_client.get_account_data(&order_pda).await {
                    Ok(data) => {
                        if data.len() < 8 {
                            error!(logger, "Order account data too short"; "order_id" => &order_id);
                            continue;
                        }
                        let mut slice = &data[8..];
                        match SvmOrder::deserialize(&mut slice) {
                            Ok(order) => order,
                            Err(e) => {
                                error!(
                                    logger,
                                    "Failed to deserialize order";
                                    "order_id" => &order_id,
                                    "error" => %e,
                                );
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            logger,
                            "Failed to fetch order account";
                            "order_id" => &order_id,
                            "error" => %e,
                        );
                        continue;
                    }
                };

                let tracked = TrackedOrder {
                    order_id: order_id.clone(),
                    origin_chain_id: chain_id,
                    sender: order.data.sender.to_string(),
                    token_in: order.data.token_in.to_string(),
                    amount_in: order.data.amount_in.to_string(),
                    dest_chain_id: order.data.dest_chain_id,
                    token_out: format!("0x{}", hex::encode(order.data.token_out)),
                    amount_out: order.data.amount_out.to_string(),
                    solver: format!("0x{}", hex::encode(order.data.solver)),
                };

                info!(logger, "Tracked new SVM order";
                    "order_id" => &order_id,
                    "chain_id" => chain_id,
                    "sender" => &tracked.sender
                );

                order_store.insert_order(tracked).await;
            }
        });

        Ok(handle)
    }

    /// Parse OrderOpened event from transaction inner instructions
    fn parse_order_opened_event(
        tx: &EncodedConfirmedTransactionWithStatusMeta,
        logger: &Logger,
        signature: &str,
    ) -> Option<OrderOpenedEvent> {
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

                    // Check for event discriminator prefix + OrderOpened discriminator
                    if decoded_data.len() < 16
                        || decoded_data[0..8] != EVENT_DISCRIMINATOR
                        || decoded_data[8..16] != ORDER_OPENED_DISCRIMINATOR
                    {
                        continue;
                    }

                    let mut data_slice = &decoded_data[16..];
                    match OrderOpenedEvent::deserialize(&mut data_slice) {
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
}
