use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::signature::Keypair;
use anchor_client::{Client, Cluster};
use async_trait::async_trait;
use m0_liquidity_sdk::types::ChainRuntime;
use order_book::{OrderData, OrderOpened};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::config::{self, ChainConfig};
use crate::error::Result;
use crate::events::{EventBus, EventHandler, EventProcessor, OrderCreatedEvent, SolverEvent};
use crate::stores::OrderStore;
use crate::utils::chain_runtime;

pub struct SvmEventListener {
    event_bus: Arc<EventBus>,
    order_store: Arc<RwLock<OrderStore>>,
    chains: Vec<ChainConfig>,
    cluster: config::Network,
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
}

impl SvmEventListener {
    pub fn new(
        event_bus: Arc<EventBus>,
        chains: Vec<ChainConfig>,
        cluster: config::Network,
    ) -> Self {
        Self {
            task_handles: Arc::new(RwLock::new(Vec::new())),
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            chains,
            cluster,
            event_bus,
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
        let store = self.order_store.read().await;
        let _ = store.handle_event(event.clone()).await;

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
    fn start_event_listener(&self, chain: &ChainConfig) {
        if chain_runtime(chain.chain_id) != ChainRuntime::Svm {
            return;
        }

        let cluster = self.cluster.clone();
        let event_bus = self.event_bus.clone();
        let chain_id = chain.chain_id.clone();
        let order_book_address = chain.order_book_address.clone();

        let handle = tokio::spawn(async move {
            let c = Cluster::from_str(&cluster.to_string()).unwrap();
            let client = Client::new(c, Arc::new(Keypair::new()));
            let chain_id_clone = chain_id.clone();

            let program = client
                .program(Pubkey::from_str(&order_book_address).unwrap())
                .unwrap();

            program
                .on::<OrderOpened>(move |_ctx, event| {
                    let order = OrderData {
                        version: 0, // TODO: Get from contract or config
                        origin_chain_id: chain_id_clone.clone(),
                        sender: [0u8; 32], // TODO: Extract from event
                        nonce: 0,          // TODO: Extract from event
                        dest_chain_id: event.dest_chain_id,
                        fill_deadline: 0, // TODO: Extract from event
                        token_out: event.token_out,
                        recipient: event.solver,
                        amount_out: 0, // TODO: Extract from event
                        solver: event.solver,
                    };

                    let order_event = OrderCreatedEvent::new(order, event.token_in.to_bytes());

                    let event_bus_clone = event_bus.clone();
                    let chain_id_for_error = chain_id.clone();
                    tokio::spawn(async move {
                        if let Err(e) = event_bus_clone
                            .publish(SolverEvent::OrderCreated(order_event))
                            .await
                        {
                            tracing::error!(
                                "Failed to publish OrderEvent on {}: {}",
                                chain_id_for_error,
                                e
                            );
                        }
                    });
                })
                .await
                .unwrap();
        });

        // Store the task handle so we can abort it later
        let task_handles = self.task_handles.clone();
        tokio::spawn(async move {
            let mut handles = task_handles.write().await;
            handles.push(handle);
        });
    }
}
