use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use m0_liquidity_sdk::types::Asset;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::events::{EventProcessor, SolverEvent};

pub struct BalanceStore {
    balances: Arc<RwLock<HashMap<Asset, u128>>>,
}

impl BalanceStore {
    pub fn new() -> Self {
        Self {
            balances: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_all_balances(&self) -> HashMap<Asset, u128> {
        self.balances.read().await.clone()
    }
}

#[async_trait]
impl EventProcessor for BalanceStore {
    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<()> {
        if let SolverEvent::InventoryUpdate(e) = event {
            let mut balances = self.balances.write().await;
            *balances = e.balances;
        }
        Ok(())
    }
}
