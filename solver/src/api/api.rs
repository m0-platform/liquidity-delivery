use nanoid::nanoid;
use poem::{listener::TcpListener, Route, Server};
use poem_openapi::{param::Query, payload::Json, OpenApi, OpenApiService};
use slog::error;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{oneshot, oneshot::Sender, Mutex};

use crate::{
    api::{
        ErrorResponse, HealthApiResponse, HealthResponse, OrderInfo, OrdersApiResponse,
        QuoteApiResponse, QuoteRequest, QuoteResponse,
    },
    components::ComponentParams,
    events::{APIRequestQuoteEvent, EventBus},
    stores::OrderStore,
    SolverEvent::APIRequestQuote,
};

#[derive(Clone)]
pub struct SolverApi {
    order_store: Arc<OrderStore>,
    logger: slog::Logger,
    event_bus: Arc<EventBus>,
    response_channels: Arc<Mutex<HashMap<String, Sender<QuoteResponse>>>>,
    port: u16,
}

#[OpenApi]
impl SolverApi {
    /// Quote
    ///
    /// Get a quote for a cross-chain swap
    #[oai(path = "/quote", method = "post")]
    async fn get_quote(&self, req: Json<QuoteRequest>) -> QuoteApiResponse {
        let mut channels = self.response_channels.lock().await;
        let (tx, rx) = oneshot::channel::<QuoteResponse>();

        let request_id = nanoid!();
        channels.insert(request_id.clone(), tx);
        drop(channels);

        // Put quote request event on the event bus
        let request = APIRequestQuote(APIRequestQuoteEvent {
            request: req.0,
            id: request_id,
        });

        if let Err(e) = self.event_bus.publish(request).await {
            error!(
                self.logger,
                "Failed to publish quote request";
                "error" => %e
            );
            return QuoteApiResponse::InternalError(Json(ErrorResponse::default()));
        }

        // Wait for the response
        match rx.await {
            Ok(quote) => QuoteApiResponse::Ok(Json(quote)),
            Err(e) => {
                error!(
                    self.logger,
                    "Failed to get quote request";
                    "error" => %e
                );
                return QuoteApiResponse::InternalError(Json(ErrorResponse::default()));
            }
        }
    }

    /// Active Orders
    ///
    /// Returns a list of all orders currently tracked by the solver.
    #[oai(path = "/orders", method = "get")]
    async fn get_orders(
        &self,
        /// Filter by order state (optional)
        state: Query<Option<String>>,
    ) -> OrdersApiResponse {
        let orders = self.order_store.get_all_orders().await;
        let mut order_infos: Vec<OrderInfo> = orders.into_iter().map(|o| o.into()).collect();

        // Filter by state if provided
        if let Some(state_filter) = state.0 {
            order_infos.retain(|o| o.state.to_lowercase() == state_filter.to_lowercase());
        }

        OrdersApiResponse::Ok(Json(order_infos))
    }

    /// Health Check
    #[oai(path = "/health", method = "get")]
    async fn get_health(&self) -> HealthApiResponse {
        HealthApiResponse::Ok(Json(HealthResponse::default()))
    }
}

impl SolverApi {
    pub fn new(params: &ComponentParams, order_store: Arc<OrderStore>) -> Self {
        let logger = params.logger.new(slog::o!("component" => "SolverApi"));

        Self {
            order_store,
            event_bus: params.event_bus.clone(),
            logger,
            port: params.config.api_server_port,
            response_channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_server(self) {
        let logger = self.logger.clone();
        let port = self.port;

        let api_service = OpenApiService::new(self, "Solver API", "1.0.0")
            .server(format!("http://localhost:{}", port));

        let ui = api_service.stoplight_elements();
        let spec = api_service.spec_endpoint();

        let route = Route::new()
            .nest("/", api_service)
            .nest("/docs", ui)
            .nest("/spec", spec);

        let server = Server::new(TcpListener::bind(format!("0.0.0.0:{}", port))).run(route);

        tokio::spawn(async move {
            if let Err(e) = server.await {
                error!(logger, "API server error"; "error" => %e);
            }
        });
    }

    pub async fn handle_quote_response(&self, response: QuoteResponse, id: String) {
        let mut channels = self.response_channels.lock().await;
        if let Some(tx) = channels.remove(&id) {
            let _ = tx.send(response);
        }
    }
}
