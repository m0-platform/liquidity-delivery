# Quoter Service

A Rust-based quoter service that provides a REST API for requesting quotes and uses gRPC for distributed quote collection from multiple solver services.

## Architecture

The service consists of two main components:

1. **HTTP API Server** (Port 3000): Receives quote requests via REST API
2. **gRPC Server** (Port 50051): Allows solver services to subscribe and respond to quote requests

## API

### POST /quote

Request a quote from all connected solvers.

**Request Body:**
```json
{
  "input_token": "0x...",
  "input_chain_id": 1,
  "output_token": "0x...",
  "output_chain_id": 42161,
  "amount_in": 1000000
}
```

**Response:**
```json
[
  {
    "quote_id": "uuid",
    "fee_bps": 10,
    "output_amount": 990000,
    "est_fill_time_seconds": 30,
    "expires_at": "2026-01-05T12:00:00Z",
    "rejected": false,
    "reject_reason": null,
    "solver_address": "0x...",
    "requires_exclusivity": false
  }
]
```

## gRPC Subscription

Solver services can connect to the gRPC server to subscribe to quote requests:

```protobuf
service QuoteService {
    rpc SubscribeToQuotes(stream QuoteResponseProto) returns (stream QuoteRequestProto);
}
```

### Flow:
1. Solver connects to gRPC server via bidirectional stream
2. Server sends quote requests to all connected solvers
3. Solvers have 500ms to respond with their quotes
4. All responses are collected and returned via the HTTP API

## Running

`protobuf` needs to be installed via `brew install protobuf`.

```bash
cargo run
```

## Testing

Example curl request:
```bash
curl -X POST http://localhost:3000/quote \
  -H "Content-Type: application/json" \
  -d '{
    "input_token": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    "input_chain_id": 1,
    "output_token": "0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8",
    "output_chain_id": 42161,
    "amount_in": 1000000
  }'
```
