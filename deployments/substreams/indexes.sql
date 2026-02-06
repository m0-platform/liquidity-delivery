-- Indexes on order_id
CREATE INDEX idx_cancel_reported_order_id ON events_cancel_reported (order_id);
CREATE INDEX idx_fill_reported_order_id ON events_fill_reported (order_id);
CREATE INDEX idx_order_cancelled_order_id ON events_order_cancelled (order_id);
CREATE INDEX idx_order_completed_order_id ON events_order_completed (order_id);
CREATE INDEX idx_order_filled_order_id ON events_order_filled (order_id);
CREATE INDEX idx_order_opened_order_id ON events_order_opened (order_id);
CREATE INDEX idx_refund_claimed_order_id ON events_refund_claimed (order_id);

-- Indexes on transaction_hash
CREATE INDEX idx_cancel_reported_tx_hash ON events_cancel_reported (transaction_hash);
CREATE INDEX idx_fill_reported_tx_hash ON events_fill_reported (transaction_hash);
CREATE INDEX idx_order_cancelled_tx_hash ON events_order_cancelled (transaction_hash);
CREATE INDEX idx_order_completed_tx_hash ON events_order_completed (transaction_hash);
CREATE INDEX idx_order_filled_tx_hash ON events_order_filled (transaction_hash);
CREATE INDEX idx_order_opened_tx_hash ON events_order_opened (transaction_hash);
CREATE INDEX idx_refund_claimed_tx_hash ON events_refund_claimed (transaction_hash);

-- Indexes on ts
CREATE INDEX idx_cancel_reported_ts ON events_cancel_reported (ts);
CREATE INDEX idx_fill_reported_ts ON events_fill_reported (ts);
CREATE INDEX idx_order_cancelled_ts ON events_order_cancelled (ts);
CREATE INDEX idx_order_completed_ts ON events_order_completed (ts);
CREATE INDEX idx_order_filled_ts ON events_order_filled (ts);
CREATE INDEX idx_order_opened_ts ON events_order_opened (ts);
CREATE INDEX idx_refund_claimed_ts ON events_refund_claimed (ts);
