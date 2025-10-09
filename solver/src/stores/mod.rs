use crate::events::EventHandler;

pub mod order_store;

pub use order_store::*;

/// Base trait for all stores
pub trait Store: Send + Sync + EventHandler {
    fn name(&self) -> &str;
}
