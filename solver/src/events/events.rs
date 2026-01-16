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
    pub transaction_hash: String,
}

impl OrderCreatedEvent {
    pub fn new(order: OrderData, transaction_hash: String) -> Self {
        Self {
            order_id: hex::encode(order.compute_order_id()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            order,
            transaction_hash,
        }
    }
}

/// Event: Order fill
#[derive(Debug, Clone)]
pub struct OrderFillEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub amount: u128,
    pub transaction_hash: String,
}

impl OrderFillEvent {
    pub fn new(order_id: String, amount: u128, transaction_hash: String) -> Self {
        Self {
            order_id,
            amount,
            transaction_hash,
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
    pub requested_at: u64,
    pub transaction_hash: String,
}

impl OrderCancelRequestEvent {
    pub fn new(order_id: String, requested_at: u64, transaction_hash: String) -> Self {
        Self {
            order_id,
            requested_at,
            transaction_hash,
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
    pub transaction_hash: String,
}

impl OrderRefundClaimedEvent {
    pub fn new(order_id: String, sender: String, amount_refunded: u128, transaction_hash: String) -> Self {
        Self {
            order_id,
            sender,
            amount_refunded,
            transaction_hash,
        }
    }
}

/// Event: Order completed
#[derive(Debug, Clone)]
pub struct OrderCompletedEvent {
    pub order_id: String,
    pub timestamp: u64,
    pub transaction_hash: String,
}

impl OrderCompletedEvent {
    pub fn new(order_id: String, transaction_hash: String) -> Self {
        Self {
            order_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            transaction_hash,
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
