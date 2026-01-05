use slog::Logger;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{timeout, Duration};
use tokio_stream::{wrappers::ReceiverStream, Stream, StreamExt};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::models::{QuoteRequest, QuoteResponse};

pub mod proto {
    tonic::include_proto!("quoter");
}

use proto::{
    quote_service_server::{QuoteService, QuoteServiceServer},
    QuoteRequestProto, QuoteResponseProto,
};

type ResponseStream = Pin<Box<dyn Stream<Item = Result<QuoteRequestProto, Status>> + Send>>;

#[derive(Clone)]
pub struct QuoteGrpcService {
    request_sender: Arc<broadcast::Sender<QuoteRequestProto>>,
    response_collectors: Arc<RwLock<HashMap<String, mpsc::Sender<QuoteResponseProto>>>>,
    quote_timeout_ms: u64,
    logger: Logger,
}

impl QuoteGrpcService {
    pub fn new(quote_timeout_ms: u64, logger: Logger) -> Self {
        let (request_sender, _) = broadcast::channel(100);
        Self {
            request_sender: Arc::new(request_sender),
            response_collectors: Arc::new(RwLock::new(HashMap::new())),
            quote_timeout_ms,
            logger: logger.new(slog::o!("component" => "QuoteGrpcService")),
        }
    }

    pub fn get_server(&self) -> QuoteServiceServer<Self> {
        QuoteServiceServer::new(self.clone())
    }

    /// Broadcast a quote request to all subscribers and collect responses
    pub async fn request_quotes(&self, request: QuoteRequest) -> Vec<QuoteResponse> {
        let request_id = Uuid::new_v4().to_string();

        let proto_request = QuoteRequestProto {
            request_id: request_id.clone(),
            input_token: request.input_token,
            input_chain_id: request.input_chain_id,
            output_token: request.output_token,
            output_chain_id: request.output_chain_id,
            amount_in: request.amount_in,
        };

        // Create a channel to collect responses for this request
        let (response_tx, mut response_rx) = mpsc::channel::<QuoteResponseProto>(100);

        {
            let mut collectors = self.response_collectors.write().await;
            collectors.insert(request_id.clone(), response_tx);
        }

        // Broadcast the request to all subscribers
        let _ = self.request_sender.send(proto_request);

        let responses: Vec<QuoteResponse> =
            match timeout(Duration::from_millis(self.quote_timeout_ms), async {
                let mut collected = Vec::new();
                while let Some(response) = response_rx.recv().await {
                    collected.push(response.into());
                }
                collected
            })
            .await
            {
                Ok(collected) => collected,
                Err(_) => {
                    // Timeout occurred, collect whatever we have
                    let mut collected = Vec::new();
                    while let Ok(response) = response_rx.try_recv() {
                        collected.push(response.into());
                    }
                    collected
                }
            };

        // Clean up the collector
        {
            let mut collectors = self.response_collectors.write().await;
            collectors.remove(&request_id);
        }

        responses
    }
}

#[tonic::async_trait]
impl QuoteService for QuoteGrpcService {
    type SubscribeToQuotesStream = ResponseStream;

    async fn subscribe_to_quotes(
        &self,
        request: Request<tonic::Streaming<QuoteResponseProto>>,
    ) -> Result<Response<Self::SubscribeToQuotesStream>, Status> {
        let mut in_stream = request.into_inner();

        let mut request_receiver = self.request_sender.subscribe();
        let response_collectors = self.response_collectors.clone();
        let logger = self.logger.clone();

        let (tx, rx) = mpsc::channel(100);

        // Spawn a task to handle incoming responses
        let logger_clone = logger.clone();
        tokio::spawn(async move {
            while let Some(result) = in_stream.next().await {
                match result {
                    Ok(response) => {
                        // Route the response to the appropriate collector
                        let collectors = response_collectors.read().await;
                        if let Some(collector) = collectors.get(&response.request_id) {
                            let _ = collector.send(response).await;
                        }
                    }
                    Err(e) => {
                        slog::error!(logger_clone, "Error receiving response from client"; "error" => %e);
                        break;
                    }
                }
            }
        });

        // Spawn a task to forward quote requests
        tokio::spawn(async move {
            loop {
                match request_receiver.recv().await {
                    Ok(request) => {
                        if tx.send(Ok(request)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        slog::warn!(
                            logger,
                            "Client lagged behind, some requests may have been missed"
                        );
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::SubscribeToQuotesStream
        ))
    }
}
