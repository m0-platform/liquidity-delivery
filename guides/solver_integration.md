# Solver Integration Guide

This document provides the technical details needed for solvers to integrate with the M0 Liquidity Delivery protocol. It covers smart contract integration, testnet deployments, and the orchestration API for receiving quote requests.

## Table of Contents

- [Overview](#overview)
- [Smart Contract Integration](#smart-contract-integration)
  - [Events to Monitor](#events-to-monitor)
  - [Filling Orders](#filling-orders)
  - [Order Data Structures](#order-data-structures)
  - [Cross-Chain Messaging](#cross-chain-messaging)
- [Testnet Deployments](#testnet-deployments)
  - [Chain ID Reference](#chain-id-reference)
  - [OrderBook Contracts](#orderbook-contracts)
  - [Portal V2 Contracts](#portal-v2-contracts)
  - [Token Addresses](#token-addresses)
- [Orchestration API Integration](#orchestration-api-integration)
  - [Architecture](#architecture)
  - [gRPC Interface](#grpc-interface)
  - [Quote Request/Response](#quote-requestresponse)

---

## Overview

The M0 Liquidity Delivery protocol is an intent-based limit order system for cross-chain token swaps. Users create orders on an origin chain, specifying a destination chain, output token, and minimum amount. Solvers monitor these orders and fill them by providing the output tokens to recipients on the destination chain.

**Solver Workflow:**

1. Monitor `OrderOpened` events on origin chains
2. Evaluate profitability and decide whether to fill
3. Call `fillOrder` on the destination chain with output tokens
4. Receive released input tokens on the origin chain after cross-chain confirmation

---

## Smart Contract Integration

### Events to Monitor

#### EVM Events

**OrderOpened** - Emitted when a new order is created on the origin chain

```solidity
event OrderOpened(
    bytes32 indexed orderId,
    address sender,
    address tokenIn,
    uint128 amountIn,
    uint32 indexed destChainId,
    bytes32 tokenOut,
    uint128 amountOut,
    bytes32 indexed solver
);
```

| Field         | Description                                                                 |
| ------------- | --------------------------------------------------------------------------- |
| `orderId`     | Unique identifier for the order                                             |
| `sender`      | Address that provided input tokens                                          |
| `tokenIn`     | Input token address on origin chain                                         |
| `amountIn`    | Amount of input tokens escrowed                                             |
| `destChainId` | Internal chain ID where order should be filled                              |
| `tokenOut`    | Output token address on destination (bytes32 for cross-chain compatibility) |
| `amountOut`   | Minimum output tokens expected by user                                      |
| `solver`      | Exclusive solver address, or zero if any solver can fill                    |

**OrderFilled** - Emitted when an order is filled on the destination chain

```solidity
event OrderFilled(
    bytes32 indexed orderId,
    address indexed solver,
    uint128 amountInToRelease,
    uint128 amountOutFilled,
    bytes32 indexed messageId
);
```

**OrderCompleted** - Emitted when an order is fully filled

```solidity
event OrderCompleted(bytes32 orderId);
```

**FillReported** - Emitted on origin chain when a fill report is received

```solidity
event FillReported(
    bytes32 indexed orderId,
    address indexed originRecipient,
    uint128 amountInToRelease,
    uint128 amountOutFilled
);
```

**OrderCancelled** - Emitted when an order is cancelled on the destination chain

```solidity
event OrderCancelled(
    bytes32 indexed orderId,
    bytes32 indexed messageId
);
```

| Field       | Description                                                                   |
| ----------- | ----------------------------------------------------------------------------- |
| `orderId`   | The order that was cancelled                                                  |
| `messageId` | Cross-chain message ID reporting cancellation to origin (zero for same-chain) |

**CancelReported** - Emitted on origin chain when a cancellation report is received

```solidity
event CancelReported(bytes32 indexed orderId);
```

#### SVM Events

On Solana, events are emitted as Anchor events. Monitor program logs for:

- `OrderOpened` - New order created
- `OrderFilled` - Order filled by solver
- `OrderCompleted` - Order fully filled
- `OrderCancelled` - Order cancelled on destination chain
- `FillReported` - Fill report received from destination
- `CancelReported` - Cancel report received from destination

### Filling Orders

#### EVM Fill Functions

```solidity
// Basic fill (same-chain or cross-chain with default adapter)
function fillOrder(
    bytes32 orderId_,
    OrderData calldata orderData_,
    FillParams calldata fillerParams_
) external payable returns (bytes32 messageId_);

// Cross-chain fill with bridge adapter arguments
function fillOrder(
    bytes32 orderId_,
    OrderData calldata orderData_,
    FillParams calldata fillerParams_,
    bytes calldata bridgeAdapterArgs_
) external payable returns (bytes32 messageId_);

// Cross-chain fill with specific bridge adapter
function fillOrder(
    bytes32 orderId_,
    OrderData calldata orderData_,
    FillParams calldata fillerParams_,
    address bridgeAdapter_,
    bytes calldata bridgeAdapterArgs_
) external payable returns (bytes32 messageId_);
```

**Parameters:**

| Parameter            | Description                                                                                |
| -------------------- | ------------------------------------------------------------------------------------------ |
| `orderId_`           | The order ID (keccak256 hash of OrderData)                                                 |
| `orderData_`         | Complete order information for verification                                                |
| `fillerParams_`      | Solver-provided fill parameters                                                            |
| `bridgeAdapter_`     | (Optional) Specific bridge adapter address                                                 |
| `bridgeAdapterArgs_` | (Optional) Bridge-specific arguments (see [Cross-Chain Messaging](#cross-chain-messaging)) |

**msg.value Requirements:**

- Same-chain fills: `0`
- Cross-chain fills: Fee for cross-chain message delivery (see bridge adapter requirements)

#### SVM Fill Instructions

```rust
// Fill order originating on Solana (same-chain)
pub fn fill_native_order(
    ctx: Context<FillNativeOrder>,
    order_id: [u8; 32],
    order_data: Box<OrderData>,
    fill_params: FillParams,
) -> Result<()>

// Fill order from another chain (cross-chain, sends fill report)
pub fn fill_foreign_order<'info>(
    ctx: Context<'_, '_, 'info, 'info, FillForeignOrder<'info>>,
    order_id: [u8; 32],
    order_data: OrderData,
    fill_params: FillParams,
) -> Result<()>
```

### Order Data Structures

#### OrderData (EVM)

```solidity
struct OrderData {
    uint16 version;        // Contract version
    bytes32 sender;        // Order creator (bytes32 for cross-chain)
    uint64 nonce;          // Unique per sender
    uint32 originChainId;  // Internal chain ID where created
    uint32 destChainId;    // Internal chain ID where filled
    uint64 createdAt;      // Creation timestamp
    uint64 fillDeadline;   // Must fill before this timestamp
    bytes32 tokenIn;       // Input token on origin chain
    bytes32 tokenOut;      // Output token on destination chain
    uint128 amountIn;      // Input token amount
    uint128 amountOut;     // Expected output amount
    bytes32 recipient;     // Receives output tokens
    bytes32 solver;        // Exclusive solver (zero = anyone)
}
```

#### FillParams (EVM)

```solidity
struct FillParams {
    uint128 amountOutToFill;   // Amount of output token to provide
    bytes32 originRecipient;   // Address to receive released input tokens
    bytes32 refundAddress;     // Address for bridge refund costs (optional)
}
```

#### OrderData (SVM)

```rust
pub struct OrderData {
    pub version: u16,           // Contract version
    pub sender: [u8; 32],       // Order creator (bytes32 for cross-chain)
    pub nonce: u64,             // Unique per sender
    pub origin_chain_id: u32,   // Internal chain ID where created
    pub dest_chain_id: u32,     // Internal chain ID where filled
    pub created_at: u64,        // Creation timestamp
    pub fill_deadline: u64,     // Must fill before this timestamp
    pub token_in: [u8; 32],     // Input token on origin chain
    pub token_out: [u8; 32],    // Output token on destination chain
    pub amount_in: u128,        // Input token amount
    pub amount_out: u128,       // Expected output amount
    pub recipient: [u8; 32],    // Receives output tokens
    pub solver: [u8; 32],       // Exclusive solver (zero = anyone)
}
```

#### FillParams (SVM)

```rust
pub struct FillParams {
    pub amount_out_to_fill: u64,    // Amount of output token to provide
    pub origin_recipient: [u8; 32], // Address to receive released input tokens
}
```

> **Note:** The SVM `FillParams` uses `u64` for `amount_out_to_fill` (matching SPL token amounts) while EVM uses `u128`. The SVM struct does not include a `refundAddress` field as refunds are handled differently on Solana.

#### Order ID Computation

Order IDs are computed identically on EVM and SVM:

```solidity
bytes32 orderId = keccak256(abi.encodePacked(
    orderData.version,
    orderData.sender,
    orderData.nonce,
    orderData.originChainId,
    orderData.destChainId,
    orderData.createdAt,
    orderData.fillDeadline,
    orderData.tokenIn,
    orderData.tokenOut,
    orderData.amountIn,
    orderData.amountOut,
    orderData.recipient,
    orderData.solver
));
```

### Cross-Chain Messaging

Cross-chain fills require sending a fill report back to the origin chain via Portal V2. The `bridgeAdapterArgs` parameter varies by bridge provider.

#### Wormhole Routes

Wormhole routes require a signed quote from the Wormhole Executor API.

**1. Get a quote from the Executor API:**

```
POST https://executor-testnet.labsapis.com/v0/quote
```

The quote includes execution cost estimates and must be obtained before calling `fillOrder`.

**Explorer:** [Wormhole Executor Explorer (Testnet)](https://wormholelabs-xyz.github.io/executor-explorer/#/?endpoint=https%3A%2F%2Fexecutor-testnet.labsapis.com&env=Testnet)

**2. Pass the signed quote as bridgeAdapterArgs:**

```solidity
bytes memory bridgeAdapterArgs = signedQuoteFromExecutorAPI;

orderBook.fillOrder{value: executionFee}(
    orderId,
    orderData,
    fillParams,
    wormholeAdapter,
    bridgeAdapterArgs
);
```

The `msg.value` should match the `estimatedCost` returned by the quote endpoint.

#### Hyperlane Routes

Hyperlane routes do not require special bridge adapter arguments. The adapter builds metadata internally. The fee is determined onchain via the adapter's `quote()` function.

**1. Get a fee quote from the adapter contract:**

```solidity
uint256 hyperlaneFee = IBridgeAdapter(hyperlaneAdapter).quote(
    destinationChainId, // Internal chain ID (e.g., 11155111 for Sepolia)
    gasLimit,           // Gas limit for destination execution (e.g., 250_000)
    payload             // The message payload
);
```

**2. Pass the fee as msg.value with empty bridgeAdapterArgs:**

```solidity
bytes memory bridgeAdapterArgs = ""; // Empty for Hyperlane

orderBook.fillOrder{value: hyperlaneFee}(
    orderId,
    orderData,
    fillParams,
    hyperlaneAdapter,
    bridgeAdapterArgs
);
```

The `msg.value` covers the Hyperlane mailbox dispatch fee.

#### LayerZero Routes

LayerZero routes work the same as Hyperlane — no special bridge adapter arguments are needed. The fee is determined onchain via the adapter's `quote()` function.

**1. Get a fee quote from the adapter contract:**

```solidity
uint256 layerZeroFee = IBridgeAdapter(layerZeroAdapter).quote(
    destinationChainId, // Internal chain ID (e.g., 11155111 for Sepolia)
    gasLimit,           // Gas limit for destination execution (e.g., 250_000)
    payload             // The message payload
);
```

**2. Pass the fee as msg.value with empty bridgeAdapterArgs:**

```solidity
bytes memory bridgeAdapterArgs = ""; // Empty for LayerZero

orderBook.fillOrder{value: layerZeroFee}(
    orderId,
    orderData,
    fillParams,
    layerZeroAdapter,
    bridgeAdapterArgs
);
```

The `msg.value` covers the LayerZero endpoint dispatch fee. Any excess is refunded to the `refundAddress` specified in `FillParams`.

#### Adapter Fee Quoting

Both Hyperlane and LayerZero adapters support onchain fee quoting via the `IBridgeAdapter.quote()` function:

```solidity
interface IBridgeAdapter {
    function quote(
        uint32 destinationChainId,
        uint256 gasLimit,
        bytes memory payload
    ) external view returns (uint256 fee);
}
```

The `gasLimit` parameter can be obtained by calling `payloadGasLimit` on the Portal V2 contract with the appropriate `PayloadType`:

```solidity
uint256 gasLimit = IPortal(portal).payloadGasLimit(destinationChainId, payloadType);
```

The `PayloadType` enum is defined in the Portal V2 `PayloadEncoder` library. The relevant types for solvers are:

| PayloadType    | Value | Usage               |
| -------------- | ----- | ------------------- |
| `FillReport`   | `4`   | Order fills         |
| `CancelReport` | `6`   | Order cancellations |

Wormhole uses a different quoting mechanism via its Executor API (see above).

---

## Testnet Deployments

### Chain ID Reference

The OrderBook uses internal chain IDs for cross-chain routing. These match the standard EVM chain IDs for EVM networks.

| Network          | Internal Chain ID | Explorer                                                       |
| ---------------- | ----------------- | -------------------------------------------------------------- |
| Sepolia          | `11155111`        | [Etherscan](https://sepolia.etherscan.io)                      |
| Arbitrum Sepolia | `421614`          | [Arbiscan](https://sepolia.arbiscan.io)                        |
| Base Sepolia     | `84532`           | [Basescan](https://sepolia.basescan.org)                       |
| Solana Devnet    | `1399811150`      | [Solana Explorer](https://explorer.solana.com/?cluster=devnet) |

### OrderBook Contracts

#### EVM

| Network          | Address                                                                                                                         |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Sepolia          | [`0xe39B012AB3b20E94a9beEa557eB0DE4171D4D3E4`](https://sepolia.etherscan.io/address/0xe39B012AB3b20E94a9beEa557eB0DE4171D4D3E4) |
| Arbitrum Sepolia | [`0xe39B012AB3b20E94a9beEa557eB0DE4171D4D3E4`](https://sepolia.arbiscan.io/address/0xe39B012AB3b20E94a9beEa557eB0DE4171D4D3E4)  |
| Base Sepolia     | [`0xe39B012AB3b20E94a9beEa557eB0DE4171D4D3E4`](https://sepolia.basescan.org/address/0xe39B012AB3b20E94a9beEa557eB0DE4171D4D3E4) |

#### SVM

| Network       | Program ID                                                                                                                                      |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Solana Devnet | [`MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK`](https://explorer.solana.com/address/MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK?cluster=devnet) |

### Portal V2 Contracts

Portal V2 handles cross-chain message delivery for fill and cancel reports.

#### Sepolia (Hub)

| Contract          | Address                                                                                                                         |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Hub Portal        | [`0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd`](https://sepolia.etherscan.io/address/0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd) |
| Hyperlane Adapter | [`0x408F6E7bDE5634160fda61b945DC9F41B965E406`](https://sepolia.etherscan.io/address/0x408F6E7bDE5634160fda61b945DC9F41B965E406) |
| Wormhole Adapter  | [`0xeAae496BcDa93cCCd3fD6ff6096347979e87B153`](https://sepolia.etherscan.io/address/0xeAae496BcDa93cCCd3fD6ff6096347979e87B153) |
| LayerZero Adapter | [`0x5206A69aa6092f7082b2F9F121F516b73233051E`](https://sepolia.etherscan.io/address/0x5206A69aa6092f7082b2F9F121F516b73233051E) |

#### Arbitrum Sepolia (Spoke)

| Contract          | Address                                                                                                                        |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Spoke Portal      | [`0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd`](https://sepolia.arbiscan.io/address/0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd) |
| Hyperlane Adapter | [`0x408F6E7bDE5634160fda61b945DC9F41B965E406`](https://sepolia.arbiscan.io/address/0x408F6E7bDE5634160fda61b945DC9F41B965E406) |
| Wormhole Adapter  | [`0xeAae496BcDa93cCCd3fD6ff6096347979e87B153`](https://sepolia.arbiscan.io/address/0xeAae496BcDa93cCCd3fD6ff6096347979e87B153) |
| LayerZero Adapter | [`0x5206A69aa6092f7082b2F9F121F516b73233051E`](https://sepolia.arbiscan.io/address/0x5206A69aa6092f7082b2F9F121F516b73233051E) |

#### Base Sepolia (Spoke)

| Contract          | Address                                                                                                                         |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Spoke Portal      | [`0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd`](https://sepolia.basescan.org/address/0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd) |
| Hyperlane Adapter | [`0x408F6E7bDE5634160fda61b945DC9F41B965E406`](https://sepolia.basescan.org/address/0x408F6E7bDE5634160fda61b945DC9F41B965E406) |
| Wormhole Adapter  | [`0xeAae496BcDa93cCCd3fD6ff6096347979e87B153`](https://sepolia.basescan.org/address/0xeAae496BcDa93cCCd3fD6ff6096347979e87B153) |
| LayerZero Adapter | [`0x5206A69aa6092f7082b2F9F121F516b73233051E`](https://sepolia.basescan.org/address/0x5206A69aa6092f7082b2F9F121F516b73233051E) |

#### Solana Devnet

| Program           | Program ID                                                                                                                                      |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Portal            | [`MzBrgc8yXBj4P16GTkcSyDZkEQZB9qDqf3fh9bByJce`](https://explorer.solana.com/address/MzBrgc8yXBj4P16GTkcSyDZkEQZB9qDqf3fh9bByJce?cluster=devnet) |
| Wormhole Adapter  | [`mzp1q2j5Hr1QuLC3KFBCAUz5aUckT6qyuZKZ3WJnMmY`](https://explorer.solana.com/address/mzp1q2j5Hr1QuLC3KFBCAUz5aUckT6qyuZKZ3WJnMmY?cluster=devnet) |
| Hyperlane Adapter | [`mZhPGteS36G7FhMTcRofLQU8ocBNAsGq7u8SKSHfL2X`](https://explorer.solana.com/address/mZhPGteS36G7FhMTcRofLQU8ocBNAsGq7u8SKSHfL2X?cluster=devnet) |

### Token Addresses

#### EVM Tokens

| Network          | Token      | Address                                                                                                                         |
| ---------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Sepolia          | $M         | [`0x866a2bf4e572cbcf37d5071a7a58503bfb36be1b`](https://sepolia.etherscan.io/address/0x866a2bf4e572cbcf37d5071a7a58503bfb36be1b) |
| Sepolia          | Wrapped $M | [`0x437cc33344a0B27A429f795ff6B469C72698B291`](https://sepolia.etherscan.io/address/0x437cc33344a0B27A429f795ff6B469C72698B291) |
| Arbitrum Sepolia | $M         | [`0x866a2bf4e572cbcf37d5071a7a58503bfb36be1b`](https://sepolia.etherscan.io/address/0x866a2bf4e572cbcf37d5071a7a58503bfb36be1b) |
| Arbitrum Sepolia | Wrapped $M | [`0x437cc33344a0B27A429f795ff6B469C72698B291`](https://sepolia.etherscan.io/address/0x437cc33344a0B27A429f795ff6B469C72698B291) |
| Base Sepolia     | $M         | [`0x866a2bf4e572cbcf37d5071a7a58503bfb36be1b`](https://sepolia.etherscan.io/address/0x866a2bf4e572cbcf37d5071a7a58503bfb36be1b) |
| Base Sepolia     | Wrapped $M | [`0x437cc33344a0B27A429f795ff6B469C72698B291`](https://sepolia.etherscan.io/address/0x437cc33344a0B27A429f795ff6B469C72698B291) |

#### SVM Tokens

| Network       | Token      | Mint Address                                                                                                                                    |
| ------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Solana Devnet | $M         | [`mzeroZRGCah3j5xEWp2Nih3GDejSBbH1rbHoxDg8By6`](https://explorer.solana.com/address/mzeroZRGCah3j5xEWp2Nih3GDejSBbH1rbHoxDg8By6?cluster=devnet) |
| Solana Devnet | Wrapped $M | [`mzeroXDoBpRVhnEXBra27qzAMdxgpWVY3DzQW7xMVJp`](https://explorer.solana.com/address/mzeroXDoBpRVhnEXBra27qzAMdxgpWVY3DzQW7xMVJp?cluster=devnet) |

---

## Orchestration API Integration

The orchestration API enables permissionless solver participation. Solvers connect via gRPC to receive quote requests and respond with pricing.

### Architecture

```
┌─────────────┐     REST      ┌──────────────────┐     gRPC      ┌─────────────┐
│   Frontend  │ ──────────────▶│  Quoter Service  │◀─────────────▶│   Solver A  │
└─────────────┘   /quote      └──────────────────┘               └─────────────┘
                                       │
                                       │ gRPC
                                       ▼
                               ┌─────────────┐                    ┌─────────────┐
                               │   Solver B  │                    │   Solver C  │
                               └─────────────┘                    └─────────────┘
```

**Flow:**

1. User requests a quote via REST API
2. Quoter service broadcasts the request to all connected solvers via gRPC
3. Solvers evaluate and respond with quotes
4. Quoter aggregates responses and returns the best quote to the user
5. User creates an order on-chain (optionally specifying a solver for exclusivity)
6. Solver monitors the chain and fills the order

### gRPC Interface

Solvers must implement a gRPC service that handles quote requests. The connection is bidirectional - the quoter service initiates requests to connected solvers.

**Endpoint:** _Coming soon_

### Quote Request/Response

#### QuoteRequest

Sent by the quoter service when a user requests pricing.

```protobuf
message QuoteRequest {
    string input_token = 1;       // EVM address or SVM mint pubkey
    uint32 input_chain_id = 2;    // Internal chain ID
    string output_token = 3;      // EVM address or SVM mint pubkey
    uint32 output_chain_id = 4;   // Internal chain ID
    uint64 amount_in = 5;         // Input amount (raw units)
    string sender_address = 6;    // User's address on origin chain
    string recipient_address = 7; // Recipient address on destination chain
}
```

#### QuoteResponse

Returned by the solver with pricing and execution details.

```protobuf
message QuoteResponse {
    string quote_id = 1;              // Unique identifier for tracking
    uint32 fee_bps = 2;               // Fee in basis points
    uint64 output_amount = 3;         // Amount user will receive
    uint64 est_fill_time_seconds = 4; // Estimated time to fill
    string expires_at = 5;            // ISO 8601 timestamp when quote expires
    bool rejected = 6;                // Whether solver rejected the request
    string reject_reason = 7;         // Reason for rejection (if rejected)
    string solver_address = 8;        // Solver's address for fills
    bool requires_exclusivity = 9;    // Whether solver requires exclusive access
}
```

#### Response Fields Explained

| Field                   | Purpose                                                             |
| ----------------------- | ------------------------------------------------------------------- |
| `quote_id`              | For tracking and correlation between quote and fill                 |
| `fee_bps`               | Helps users understand the cost breakdown beyond just output amount |
| `output_amount`         | The amount the user will receive (may include slippage/impact)      |
| `est_fill_time_seconds` | Solvers may need time to source assets or segment large orders      |
| `expires_at`            | Informs frontends when to refresh quotes for dynamic pricing        |
| `rejected`              | Quick boolean check for quote acceptance                            |
| `reject_reason`         | User-friendly explanation (e.g., "Unsupported token pair")          |
| `solver_address`        | Allows users to specify exclusivity if desired                      |
| `requires_exclusivity`  | Indicates if solver needs exclusive access to avoid being front-run |

#### Important Notes

- **Quotes are non-binding**: A quote indicates willingness to fill but does not guarantee execution
- **Users should set appropriate `min_amount_out`**: The quote helps users set expectations, but market conditions may change
- **Exclusivity is optional**: Solvers can request exclusivity if sourcing assets incurs risk or cost

### Implementing a Solver Service

1. **Connect to the gRPC endpoint** (when available)
2. **Handle `QuoteRequest` messages** by evaluating:
   - Supported token pairs and chains
   - Available liquidity
   - Current market prices
   - Estimated execution costs
3. **Return `QuoteResponse`** with:
   - Competitive pricing
   - Realistic fill time estimates
   - Appropriate exclusivity requirements
4. **Monitor on-chain events** for orders matching your quotes
5. **Execute fills** by calling `fillOrder` with appropriate tokens and bridge fees

---

## Additional Resources

- [M0 Documentation](https://docs.m0.org)
- [Wormhole Executor Documentation](https://wormhole.com/docs/products/messaging/concepts/executor-overview/)
- [Wormhole Executor Explorer (Testnet)](https://wormholelabs-xyz.github.io/executor-explorer/#/?endpoint=https%3A%2F%2Fexecutor-testnet.labsapis.com&env=Testnet)
- [LayerZero V2 Documentation](https://docs.layerzero.network/v2)
