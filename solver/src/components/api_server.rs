use async_trait::async_trait;
use std::sync::Arc;

use crate::{
    api::api::SolverApi,
    error::Result,
    events::{EventHandler, EventProcessor, SolverEvent},
    stores::OrderStore,
};

use super::ComponentParams;

pub struct ApiServer {
    order_store: Arc<OrderStore>,
    api: SolverApi,
}

impl ApiServer {
    pub fn new(params: &ComponentParams) -> Self {
        let order_store = Arc::new(OrderStore::new());
        let api = SolverApi::new(params, order_store.clone());
        Self { order_store, api }
    }
}

#[async_trait]
impl EventHandler for ApiServer {
    async fn initialize(&self) -> Result<()> {
        self.order_store.initialize().await?;
        self.api.clone().start_server();
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let _ = self.order_store.handle_event(event.clone()).await;

        match event {
            SolverEvent::APIQuoteResponse(quote) => {
                self.api
                    .handle_quote_response(quote.response, quote.id)
                    .await;
            }
            _ => {}
        }

        Ok(Vec::new())
    }

    fn name(&self) -> &'static str {
        "ApiServer"
    }
}
