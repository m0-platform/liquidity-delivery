CREATE TABLE events_cancel_reported (
    id TEXT NOT NULL,
    ts NUMERIC NOT NULL,
    transaction_hash TEXT NOT NULL,
    order_id TEXT NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE events_fill_reported (
    id TEXT NOT NULL,
    ts NUMERIC NOT NULL,
    transaction_hash TEXT NOT NULL,
    order_id TEXT NOT NULL,
    amount_in_to_release NUMERIC NOT NULL,
    amount_out_filled NUMERIC NOT NULL,
    origin_recipient TEXT NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE events_order_cancelled (
    id TEXT NOT NULL,
    ts NUMERIC NOT NULL,
    transaction_hash TEXT NOT NULL,
    order_id TEXT NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE events_order_completed (
    id TEXT NOT NULL,
    ts NUMERIC NOT NULL,
    transaction_hash TEXT NOT NULL,
    order_id TEXT NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE events_order_filled (
    id TEXT NOT NULL,
    ts NUMERIC NOT NULL,
    transaction_hash TEXT NOT NULL,
    order_id TEXT NOT NULL,
    solver TEXT NOT NULL,
    amount_in_to_release NUMERIC NOT NULL,
    amount_out_filled NUMERIC NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE events_order_opened (
    id TEXT NOT NULL,
    ts NUMERIC NOT NULL,
    transaction_hash TEXT NOT NULL,
    order_id TEXT NOT NULL,
    sender TEXT NOT NULL,
    token_in TEXT NOT NULL,
    amount_in NUMERIC NOT NULL,
    dest_chain_id INTEGER NOT NULL,
    token_out TEXT NOT NULL,
    amount_out NUMERIC NOT NULL,
    solver TEXT NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE events_refund_claimed (
    id TEXT NOT NULL,
    ts NUMERIC NOT NULL,
    transaction_hash TEXT NOT NULL,
    order_id TEXT NOT NULL,
    sender TEXT NOT NULL,
    amount NUMERIC NOT NULL,
    PRIMARY KEY (id)
);

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
