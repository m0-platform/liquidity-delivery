use async_trait::async_trait;
use nanoid::nanoid;
use slog::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::transport::Channel;

use crate::{
    error::{Result, SolverError},
    events::{EventHandler, QuoteRequest, QuoteResponse, RequestQuoteEvent, SolverEvent},
    utils::decode_address,
};

use super::ComponentParams;

pub mod proto {
    tonic::include_proto!("quoter");
}

use proto::{quote_service_client::QuoteServiceClient, QuoteResponseProto};

pub struct QuoterClient {
    quoter_url: String,
    logger: slog::Logger,
    solver_fee_bps: u32,
    event_bus: Arc<crate::events::EventBus>,
    response_channels: Arc<Mutex<HashMap<String, oneshot::Sender<QuoteResponse>>>>,
    connect: bool,
}

impl QuoterClient {
    pub fn new(params: &ComponentParams) -> Self {
        let logger = params.logger.new(slog::o!("component" => "QuoterClient"));

        Self {
            quoter_url: params.config.quoter_grpc_url.clone(),
            logger,
            solver_fee_bps: params.config.solver_fee_bps,
            event_bus: params.event_bus.clone(),
            response_channels: Arc::new(Mutex::new(HashMap::new())),
            connect: params.config.connect_to_quote_stream,
        }
    }

    async fn connect(&self) -> Result<QuoteServiceClient<Channel>> {
        let client = QuoteServiceClient::connect(self.quoter_url.clone())
            .await
            .map_err(|e| SolverError::Component(format!("gRPC connection failed: {}", e)))?;

        info!(self.logger, "Connected to quoter gRPC service"; "url" => %self.quoter_url);

        Ok(client)
    }

    async fn subscribe_and_handle_quotes(&self) -> Result<()> {
        let mut client = self.connect().await?;

        let (response_tx, response_rx) = mpsc::channel::<QuoteResponseProto>(100);
        let response_stream = ReceiverStream::new(response_rx);

        let mut request_stream = client
            .subscribe_to_quotes(response_stream)
            .await
            .map_err(|e| SolverError::Component(format!("gRPC subscription failed: {}", e)))?
            .into_inner();

        let logger = self.logger.clone();
        let event_bus = self.event_bus.clone();
        let response_channels = self.response_channels.clone();

        // Handle incoming quote requests
        while let Some(request_result) = request_stream.next().await {
            match request_result {
                Ok(request) => {
                    let request_id = request.request_id.clone();
                    let response_tx_clone = response_tx.clone();
                    let logger_clone = logger.clone();
                    let event_bus_clone = event_bus.clone();
                    let response_channels_clone = response_channels.clone();

                    // Spawn task to handle this request
                    tokio::spawn(async move {
                        let create_rejection = |reason: &str| QuoteResponseProto {
                            request_id: request_id.clone(),
                            quote_id: nanoid!(),
                            rejected: true,
                            reject_reason: reason.to_string(),
                            ..Default::default()
                        };

                        // Parse input and output tokens
                        let input_asset = match decode_address(
                            request.input_token.clone(),
                            request.input_chain_id,
                        ) {
                            Some(asset) => asset,
                            None => {
                                let _ = response_tx_clone
                                    .send(create_rejection("Invalid input token"))
                                    .await;
                                return;
                            }
                        };

                        let output_asset = match decode_address(
                            request.output_token.clone(),
                            request.output_chain_id,
                        ) {
                            Some(asset) => asset,
                            None => {
                                let _ = response_tx_clone
                                    .send(create_rejection("Invalid output token"))
                                    .await;
                                return;
                            }
                        };

                        // Create oneshot channel for response
                        let (tx, rx) = oneshot::channel::<QuoteResponse>();
                        {
                            let mut channels = response_channels_clone.lock().await;
                            channels.insert(request_id.clone(), tx);
                        }

                        // Create and publish APIRequestQuote event
                        let quote_request = QuoteRequest {
                            input_token: request.input_token,
                            input_chain_id: request.input_chain_id,
                            output_token: request.output_token,
                            output_chain_id: request.output_chain_id,
                            amount_in: request.amount_in,
                        };

                        let api_event = SolverEvent::RequestQuote(RequestQuoteEvent {
                            request: quote_request,
                            id: request_id.clone(),
                            parsed_input_token: input_asset,
                            parsed_output_token: output_asset,
                        });

                        if let Err(e) = event_bus_clone.publish(api_event).await {
                            error!(logger_clone, "Failed to publish quote request"; "error" => %e);
                            let _ = response_tx_clone
                                .send(create_rejection("Internal error"))
                                .await;
                            return;
                        }

                        // Wait for response from event bus
                        match rx.await {
                            Ok(quote) => {
                                let response = QuoteResponseProto {
                                    request_id: request_id.clone(),
                                    quote_id: quote.quote_id,
                                    fee_bps: quote.fee_bps,
                                    output_amount: quote.output_amount,
                                    est_fill_time_seconds: quote.est_fill_time_seconds,
                                    expires_at: quote.expires_at,
                                    rejected: quote.rejected,
                                    reject_reason: quote.reject_reason.unwrap_or_default(),
                                    solver_address: quote.solver_address,
                                    requires_exclusivity: quote.requires_exclusivity,
                                };
                                if let Err(e) = response_tx_clone.send(response).await {
                                    error!(logger_clone, "Failed to send quote response"; "error" => %e);
                                }
                            }
                            Err(e) => {
                                error!(logger_clone, "Failed to receive quote response"; "error" => %e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!(logger, "Error receiving quote request"; "error" => %e);
                    break;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandler for QuoterClient {
    async fn initialize(&self) -> Result<()> {
        if !self.connect {
            warn!(
                self.logger,
                "QuoterClient not configured to connect to quote stream"
            );
            return Ok(());
        }

        let self_clone = Self {
            quoter_url: self.quoter_url.clone(),
            logger: self.logger.clone(),
            solver_fee_bps: self.solver_fee_bps,
            event_bus: self.event_bus.clone(),
            response_channels: self.response_channels.clone(),
            connect: self.connect,
        };

        // Spawn the subscription handler
        tokio::spawn(async move {
            loop {
                if let Err(e) = self_clone.subscribe_and_handle_quotes().await {
                    error!(self_clone.logger, "Quote subscription error"; "error" => %e);
                }

                // Wait before reconnecting
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                info!(self_clone.logger, "Attempting to reconnect to quoter");
            }
        });

        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        match event {
            SolverEvent::QuoteResponse(quote) => {
                let mut channels = self.response_channels.lock().await;
                if let Some(tx) = channels.remove(&quote.id) {
                    let _ = tx.send(quote.response);
                }
            }
            _ => {}
        }
        Ok(Vec::new())
    }

    fn name(&self) -> &'static str {
        "QuoterClient"
    }
}
