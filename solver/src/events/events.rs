use m0_liquidity_sdk::types::Asset;
use order_book::OrderData;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::{QuoteRequest, QuoteResponse};

/// Unified event enum
#[derive(Debug, Clone)]
pub enum SolverEvent {
    // System Events
    Start,
    Stop,
    Heartbeat(u128),

    // Order events
    OrderCreated(OrderCreatedEvent),
    OrderFill(OrderFillEvent),
    OrderRejected(OrderRejectEvent),
    OrderCancelRequest(OrderCancelRequestEvent),
    OrderRefundClaimed(OrderRefundClaimedEvent),
    OrderCompleted(OrderCompletedEvent),

    // Inventory events
    RequestHold(RequestHoldEvent),
    HoldSuccessful(HoldSuccessfulEvent),

    // Chain events
    RequestFillOrder(RequestFillOrderEvent),
    FillOrderSuccessful(FillOrderSuccessfulEvent),
    RequestSwap(RequestSwapEvent),
    SwapSuccessful(SwapSuccessfulEvent),

    // API events
    APIRequestQuote(APIRequestQuoteEvent),
    APIQuoteResponse(APIQuoteResponseEvent),
}

impl SolverEvent {
    /// Get the order_id from events that contain one
    pub fn order_id(&self) -> Option<String> {
        match self {
            SolverEvent::OrderCreated(e) => Some(e.order_id.clone()),
            SolverEvent::OrderFill(e) => Some(e.order_id.clone()),
            SolverEvent::OrderRejected(e) => Some(e.order_id.clone()),
            SolverEvent::OrderCancelRequest(e) => Some(e.order_id.clone()),
            SolverEvent::OrderRefundClaimed(e) => Some(e.order_id.clone()),
            SolverEvent::OrderCompleted(e) => Some(e.order_id.clone()),
            SolverEvent::RequestHold(e) => Some(e.order_id.clone()),
            SolverEvent::HoldSuccessful(e) => Some(e.order_id.clone()),
            SolverEvent::RequestFillOrder(e) => Some(e.order_id.clone()),
            SolverEvent::FillOrderSuccessful(e) => Some(e.order_id.clone()),
            _ => None,
        }
    }
}

/// Event: New order created
#[derive(Debug, Clone)]
pub struct OrderCreatedEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub order: OrderData,
}

impl OrderCreatedEvent {
    pub fn new(order: OrderData) -> Self {
        Self {
            order_id: hex::encode(order.compute_order_id()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            order,
        }
    }
}

/// Event: Order fill
#[derive(Debug, Clone)]
pub struct OrderFillEvent {
    pub order_id: String,
    pub amount: u128,
}

impl OrderFillEvent {
    pub fn new(order_id: String, amount: u128) -> Self {
        Self { order_id, amount }
    }
}

/// Event: Order Rejected
#[derive(Debug, Clone)]
pub struct OrderRejectEvent {
    pub order_id: String,
    pub reason: String,
}

impl OrderRejectEvent {
    pub fn new(order_id: String, reason: String) -> Self {
        Self { order_id, reason }
    }
}

/// Event: Order cancel requested
#[derive(Debug, Clone)]
pub struct OrderCancelRequestEvent {
    pub order_id: String,
    pub requested_at: u64,
}

impl OrderCancelRequestEvent {
    pub fn new(order_id: String, requested_at: u64) -> Self {
        Self {
            order_id,
            requested_at,
        }
    }
}

/// Event: Order refund claimed
#[derive(Debug, Clone)]
pub struct OrderRefundClaimedEvent {
    pub order_id: String,
    pub sender: String,
    pub amount_refunded: u128,
}

impl OrderRefundClaimedEvent {
    pub fn new(order_id: String, sender: String, amount_refunded: u128) -> Self {
        Self {
            order_id,
            sender,
            amount_refunded,
        }
    }
}

/// Event: Order completed
#[derive(Debug, Clone)]
pub struct OrderCompletedEvent {
    pub order_id: String,
    pub timestamp: u64,
}

impl OrderCompletedEvent {
    pub fn new(order_id: String) -> Self {
        Self {
            order_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Event: Request hold or rebalance on asset
#[derive(Debug, Clone)]
pub struct RequestHoldEvent {
    pub order_id: String,
    pub asset: Asset,
    pub amount: u128,
    pub allow_partial_hold: bool,
}

impl RequestHoldEvent {
    pub fn new(order_id: String, asset: Asset, amount: u128, allow_partial_hold: bool) -> Self {
        Self {
            order_id,
            asset,
            amount,
            allow_partial_hold,
        }
    }
}

/// Event: Asset hold successful
#[derive(Debug, Clone)]
pub struct HoldSuccessfulEvent {
    pub order_id: String,
    pub hold_amount: u128,
}

impl HoldSuccessfulEvent {
    pub fn new(order_id: String, hold_amount: u128) -> Self {
        Self {
            order_id,
            hold_amount,
        }
    }
}

/// Event: Request an order to be filled
#[derive(Debug, Clone)]
pub struct RequestFillOrderEvent {
    pub order_id: String,
    pub amount: u128,
}

impl RequestFillOrderEvent {
    pub fn new(order_id: String, amount: u128) -> Self {
        Self { order_id, amount }
    }
}

/// Event: Asset hold successful
#[derive(Debug, Clone)]
pub struct FillOrderSuccessfulEvent {
    pub order_id: String,
}

impl FillOrderSuccessfulEvent {
    pub fn new(order_id: String) -> Self {
        Self { order_id }
    }
}

/// Event: Request an order to be filled
#[derive(Debug, Clone)]
pub struct RequestSwapEvent {
    pub order_id: String,
    pub token_in: Asset,
    pub token_out: Asset,
    pub amount_in: u128,
}

impl RequestSwapEvent {
    pub fn new(order_id: String, token_in: Asset, token_out: Asset, amount_in: u128) -> Self {
        Self {
            order_id,
            token_in,
            token_out,
            amount_in,
        }
    }
}

/// Event: Asset hold successful
#[derive(Debug, Clone)]
pub struct SwapSuccessfulEvent {
    pub order_id: String,
}

#[derive(Debug, Clone)]
pub struct APIRequestQuoteEvent {
    pub request: QuoteRequest,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct APIQuoteResponseEvent {
    pub response: QuoteResponse,
    pub id: String,
}
