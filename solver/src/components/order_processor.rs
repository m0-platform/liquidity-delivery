use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::events::{EventHandler, SolverEvent};
use crate::stores::OrderStore;

pub struct OrderProcessor {
    order_store: Arc<RwLock<OrderStore>>,
}

impl OrderProcessor {
    pub fn new() -> Self {
        Self {
            order_store: Arc::new(RwLock::new(OrderStore::new())),
        }
    }
}

#[async_trait]
impl EventHandler for OrderProcessor {
    fn name(&self) -> &'static str {
        "OrderProcessor"
    }

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn handle_event(&self, _event: Arc<SolverEvent>) -> Result<Arc<Vec<SolverEvent>>> {
        Ok(Arc::new(vec![]))
    }
}
