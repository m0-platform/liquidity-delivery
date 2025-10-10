use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::signature::Keypair;
use anchor_client::{Client, Cluster};
use async_trait::async_trait;
use order_book::{OrderData, OrderOpened};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

use crate::components::Component;
use crate::config::{self, ChainConfig};
use crate::error::Result;
use crate::events::{EventBus, EventHandler, OrderCreatedEvent, OrderEvent};
use crate::stores::OrderStore;

pub struct SvmEventListener {
    order_store: Arc<RwLock<OrderStore>>,
    chains: Vec<ChainConfig>,
    cluster: config::Network,
}

impl SvmEventListener {
    pub fn new(chains: Vec<ChainConfig>, cluster: config::Network) -> Self {
        Self {
            order_store: Arc::new(RwLock::new(OrderStore::new())),
            chains,
            cluster,
        }
    }
}

#[async_trait]
impl Component for SvmEventListener {
    fn name() -> &'static str {
        "SvmEventListener"
    }

    async fn initialize(&self) -> Result<()> {
        tracing::info!(
            "Initializing SvmEventListener for {} chains",
            self.chains.len()
        );
        Ok(())
    }

    async fn start(&self, event_bus: Arc<EventBus>, shutdown_rx: Receiver<()>) -> Result<()> {
        tracing::info!("Starting SvmEventListener");

        // Task to handle events (update stores)
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
            let cluster = self.cluster.to_string();

            tokio::spawn(async move {
                let c = Cluster::from_str(cluster).unwrap();
                let client = Client::new(c, Arc::new(Keypair::new()));
                let chain_id = chain.chain_id.clone();

                let program = client
                    .program(Pubkey::from_str(&chain.order_book_address).unwrap())
                    .unwrap();

                let unsub = program
                    .on::<OrderOpened>(move |ctx, event| {
                        tracing::info!(
                            "OrderOpen event on chain {}: orderId={:?}: signature: {}",
                            chain_id.clone(),
                            event.order_id,
                            ctx.signature,
                        );

                        let order = OrderData {
                            version: 0, // TODO: Get from contract or config
                            origin_chain_id: chain_id,
                            sender: [0u8; 32], // TODO: Extract from event
                            nonce: 0,          // TODO: Extract from event
                            dest_chain_id: event.dest_chain_id,
                            fill_deadline: 0, // TODO: Extract from event
                            token_out: event.token_out,
                            recipient: event.solver,
                            amount_out: 0, // TODO: Extract from event
                            solver: event.solver,
                        };

                        let order_event = OrderCreatedEvent::new(order);

                        let event_bus_clone = chain_event_bus.clone();
                        tokio::spawn(async move {
                            if let Err(e) = event_bus_clone
                                .publish(Arc::new(OrderEvent::Created(order_event)))
                                .await
                            {
                                tracing::error!(
                                    "Failed to start listener for chain {}: {}",
                                    chain.chain_id,
                                    e
                                );
                            }
                        });
                    })
                    .await;
            });
        }

        Ok(())
    }
}
