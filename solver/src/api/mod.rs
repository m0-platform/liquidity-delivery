pub mod api;

use poem_openapi::{payload::Json, ApiResponse, Object};

use crate::stores::Order;

/// Quote request structure
#[derive(Debug, Object, Clone)]
pub struct QuoteRequest {
    /// EVM or SVM token address
    #[oai(validator(pattern = "[0-9A-HJ-NP-Za-km-z]{32,44}"))]
    pub input_token: String,
    pub input_chain_id: u32,
    /// EVM or SVM token address
    #[oai(validator(pattern = "[0-9A-HJ-NP-Za-km-z]{32,44}"))]
    pub output_token: String,
    pub output_chain_id: u32,
    pub amount_in: u64,
    pub amount_out: u64,
}

/// Quote response structure
#[derive(Debug, Object, Clone)]
pub struct QuoteResponse {
    pub can_process: bool,
    pub estimated_time_seconds: u64,
    pub reason: Option<String>,
}

impl Default for QuoteResponse {
    fn default() -> Self {
        Self {
            can_process: false,
            estimated_time_seconds: 0,
            reason: None,
        }
    }
}

/// Order information for API responses
#[derive(Debug, Object, Clone)]
pub struct OrderInfo {
    pub id: String,
    pub state: String,
    pub filled_amount: u64,
    pub amount: u64,
    pub input_token: String,
    pub output_token: String,
    pub input_chain_id: u32,
    pub output_chain_id: u32,
}

impl From<Order> for OrderInfo {
    fn from(order: Order) -> Self {
        Self {
            id: order.id,
            state: order.state.to_string(),
            filled_amount: order.filled_amount as u64,
            amount: order.data.amount_out as u64,
            input_token: hex::encode(order.data.token_in),
            output_token: hex::encode(order.data.token_out),
            input_chain_id: order.data.origin_chain_id,
            output_chain_id: order.data.dest_chain_id,
        }
    }
}

#[derive(ApiResponse)]
pub enum QuoteApiResponse {
    #[oai(status = 200)]
    Ok(Json<QuoteResponse>),
    #[oai(status = 400)]
    BadRequest(Json<ErrorResponse>),
    #[oai(status = 500)]
    InternalError(Json<ErrorResponse>),
}

#[derive(ApiResponse)]
pub enum OrdersApiResponse {
    #[oai(status = 200)]
    Ok(Json<Vec<OrderInfo>>),
    #[oai(status = 500)]
    InternalError(Json<ErrorResponse>),
}

#[derive(ApiResponse)]
pub enum HealthApiResponse {
    #[oai(status = 200)]
    Ok(Json<HealthResponse>),
    #[oai(status = 500)]
    InternalError(Json<ErrorResponse>),
}

#[derive(Debug, Object)]
pub struct HealthResponse {
    message: String,
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self {
            message: "Healthy".to_string(),
        }
    }
}

#[derive(Debug, Object)]
pub struct ErrorResponse {
    error: String,
}

impl Default for ErrorResponse {
    fn default() -> Self {
        Self {
            error: "An unknown server error occurred".to_string(),
        }
    }
}
