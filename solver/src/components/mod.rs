pub mod event_logger;
pub mod evm_event_listener;
pub mod evm_writer;
pub mod inventory_manager;
pub mod order_processor;
pub mod order_timer;
pub mod quoter_client;
pub mod svm_event_listener;

use std::sync::Arc;

#[allow(unused_imports)]
pub use event_logger::EventLogger;
pub use evm_event_listener::EvmEventListener;
pub use evm_writer::EvmWriter;
pub use inventory_manager::InventoryManager;
pub use order_processor::OrderProcessor;
pub use order_timer::OrderTimer;
pub use quoter_client::QuoterClient;
pub use svm_event_listener::SvmEventListener;

use crate::{events::EventBus, providers::ProviderManager, Config};

#[derive(Clone)]
pub struct ComponentParams {
    pub event_bus: Arc<EventBus>,
    pub config: Config,
    pub provider_manager: Arc<ProviderManager>,
    pub logger: slog::Logger,
}
