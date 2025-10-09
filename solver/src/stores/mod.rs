use async_trait::async_trait;

use crate::error::Result;
use crate::events::EventHandler;

pub mod asset_store;
pub mod order_store;

pub use asset_store::*;
pub use order_store::*;

/// Base trait for all stores
#[async_trait]
pub trait Store: Send + Sync + EventHandler {
    fn name(&self) -> &str;

    async fn initialize(&self) -> Result<()>;
}
