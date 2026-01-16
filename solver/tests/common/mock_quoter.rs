#![allow(dead_code)]

use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_stream::{Stream, StreamExt};
use tonic::{transport::Server, Request, Response, Status};

pub mod proto {
    tonic::include_proto!("quoter");
}

use proto::{
    quote_service_server::{QuoteService, QuoteServiceServer},
    QuoteRequestProto, QuoteResponseProto,
};

/// Mock gRPC quoter server for testing
pub struct MockQuoterServer {
    /// Sender to broadcast quote requests to connected solvers
    request_sender: broadcast::Sender<QuoteRequestProto>,
    /// Receiver for quote responses from solvers
    response_rx: Arc<Mutex<mpsc::Receiver<QuoteResponseProto>>>,
    /// Sender for quote responses (used internally)
    response_tx: mpsc::Sender<QuoteResponseProto>,
}

impl MockQuoterServer {
    pub fn new() -> Self {
        let (request_sender, _) = broadcast::channel(100);
        let (response_tx, response_rx) = mpsc::channel(100);

        Self {
            request_sender,
            response_rx: Arc::new(Mutex::new(response_rx)),
            response_tx,
        }
    }

    /// Start the mock server and return a handle for test interactions
    pub async fn start(self) -> MockQuoterHandle {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

        let request_sender = self.request_sender.clone();
        let response_rx = self.response_rx.clone();

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        let service = MockQuoteService {
            request_sender: request_sender.clone(),
            response_tx: self.response_tx.clone(),
        };

        tokio::spawn(async move {
            Server::builder()
                .add_service(QuoteServiceServer::new(service))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .expect("Mock gRPC server failed");
        });

        // Give the server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        MockQuoterHandle {
            addr: bound_addr,
            request_sender,
            response_rx,
        }
    }
}

/// Handle for interacting with the mock quoter in tests
pub struct MockQuoterHandle {
    pub addr: SocketAddr,
    request_sender: broadcast::Sender<QuoteRequestProto>,
    response_rx: Arc<Mutex<mpsc::Receiver<QuoteResponseProto>>>,
}

impl MockQuoterHandle {
    /// Get the gRPC URL for connecting to this mock server
    pub fn grpc_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Send a quote request to all connected solvers
    pub fn send_quote_request(&self, request: QuoteRequestProto) -> Result<(), String> {
        self.request_sender
            .send(request)
            .map(|_| ())
            .map_err(|e| format!("Failed to send quote request: {}", e))
    }

    /// Wait for a quote response from a solver with timeout
    pub async fn recv_quote_response(
        &self,
        timeout_ms: u64,
    ) -> Result<QuoteResponseProto, String> {
        let timeout = tokio::time::Duration::from_millis(timeout_ms);

        tokio::select! {
            response = async {
                self.response_rx.lock().await.recv().await
            } => {
                response.ok_or_else(|| "Response channel closed".to_string())
            }
            _ = tokio::time::sleep(timeout) => {
                Err("Timeout waiting for quote response".to_string())
            }
        }
    }
}

/// Internal service implementation
struct MockQuoteService {
    request_sender: broadcast::Sender<QuoteRequestProto>,
    response_tx: mpsc::Sender<QuoteResponseProto>,
}

#[tonic::async_trait]
impl QuoteService for MockQuoteService {
    type SubscribeToQuotesStream =
        Pin<Box<dyn Stream<Item = Result<QuoteRequestProto, Status>> + Send>>;

    async fn subscribe_to_quotes(
        &self,
        request: Request<tonic::Streaming<QuoteResponseProto>>,
    ) -> Result<Response<Self::SubscribeToQuotesStream>, Status> {
        let mut response_stream = request.into_inner();
        let response_tx = self.response_tx.clone();

        // Spawn task to forward responses from solver to test handler
        tokio::spawn(async move {
            while let Some(result) = response_stream.next().await {
                match result {
                    Ok(response) => {
                        let _ = response_tx.send(response).await;
                    }
                    Err(e) => {
                        eprintln!("Error receiving response from solver: {}", e);
                        break;
                    }
                }
            }
        });

        // Create a stream that forwards requests from test handler to solver
        let mut request_rx = self.request_sender.subscribe();

        let output_stream = async_stream::stream! {
            while let Ok(request) = request_rx.recv().await {
                yield Ok(request);
            }
        };

        Ok(Response::new(Box::pin(output_stream)))
    }
}
