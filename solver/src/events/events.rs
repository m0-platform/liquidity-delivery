use m0_liquidity_sdk::types::Asset;
use order_book::OrderData;
use std::time::{SystemTime, UNIX_EPOCH};

/// Unified event enum
#[derive(Debug, Clone)]
pub enum SolverEvent {
    // System Events
    Start,
    Stop,

    // Order events
    OrderCreated(OrderCreatedEvent),
    OrderFill(OrderFillEvent),
    OrderRejected(OrderRejectEvent),
    OrderCancelRequest(OrderCancelRequestEvent),
    OrderRefundClaimed(OrderRefundClaimedEvent),
    OrderCompleted(OrderCompletedEvent),

    // Inventory events
    RequestRebalance(RequestRebalance),
}

/// Event: New order created
#[derive(Debug, Clone)]
pub struct OrderCreatedEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub order: OrderData,
    pub token_in: [u8; 32],
}

impl OrderCreatedEvent {
    pub fn new(order: OrderData, token_in: [u8; 32]) -> Self {
        Self {
            order_id: hex::encode(order.compute_order_id()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            order,
            token_in,
        }
    }
}

/// Event: Order fill
#[derive(Debug, Clone)]
pub struct OrderFillEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub amount: u128,
}

impl OrderFillEvent {
    pub fn new(order_id: String, amount: u128) -> Self {
        Self {
            order_id,
            amount,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Event: Order Rejected
#[derive(Debug, Clone)]
pub struct OrderRejectEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub reason: String,
}

impl OrderRejectEvent {
    pub fn new(order_id: String, reason: String) -> Self {
        Self {
            order_id,
            reason,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Event: Order cancel requested
#[derive(Debug, Clone)]
pub struct OrderCancelRequestEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub requested_at: u64,
}

impl OrderCancelRequestEvent {
    pub fn new(order_id: String, requested_at: u64) -> Self {
        Self {
            order_id,
            requested_at,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Event: Order refund claimed
#[derive(Debug, Clone)]
pub struct OrderRefundClaimedEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub sender: String,
    pub amount_refunded: u128,
}

impl OrderRefundClaimedEvent {
    pub fn new(order_id: String, sender: String, amount_refunded: u128) -> Self {
        Self {
            order_id,
            sender,
            amount_refunded,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
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

/// Event: Request inventory rebalance
#[derive(Debug, Clone)]
pub struct RequestRebalance {
    pub target_order_id: String,
    pub timestamp: u64,
    pub asset: Asset,
    pub amount: u128,
}
