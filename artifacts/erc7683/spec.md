# ERC-7683 Wrapper Specification

This document specifies the design for an ERC-7683 wrapper contract that provides standard cross-chain intent interfaces on top of the M0 OrderBook.

## Overview

ERC-7683 defines a standard interface for cross-chain intents, enabling interoperability between different intent-based systems. This wrapper will implement:

- **IOriginSettler**: For opening orders on the origin chain
- **IDestinationSettler**: For filling orders on the destination chain

## ERC-7683 Interface Definitions

### Core Structs

```solidity
struct OnchainCrossChainOrder {
    uint32 fillDeadline;
    bytes32 orderDataType;
    bytes orderData;
}

struct GaslessCrossChainOrder {
    address originSettler;
    address user;
    uint256 nonce;
    uint256 originChainId;
    uint32 openDeadline;
    uint32 fillDeadline;
    bytes32 orderDataType;
    bytes orderData;
}

struct ResolvedCrossChainOrder {
    address user;
    uint256 originChainId;
    uint32 openDeadline;
    uint32 fillDeadline;
    bytes32 orderId;
    Output[] maxSpent;
    Output[] minReceived;
    FillInstruction[] fillInstructions;
}

struct Output {
    bytes32 token;
    uint256 amount;
    bytes32 recipient;
    uint256 chainId;
}

struct FillInstruction {
    uint256 destinationChainId;
    bytes32 destinationSettler;
    bytes originData;
}
```

### IOriginSettler Interface

```solidity
interface IOriginSettler {
    event Open(bytes32 indexed orderId, ResolvedCrossChainOrder resolvedOrder);

    function open(OnchainCrossChainOrder calldata order) external;
    function openFor(
        GaslessCrossChainOrder calldata order,
        bytes calldata signature,
        bytes calldata originFillerData
    ) external;
    function resolve(OnchainCrossChainOrder calldata order)
        external view returns (ResolvedCrossChainOrder memory);
    function resolveFor(
        GaslessCrossChainOrder calldata order,
        bytes calldata originFillerData
    ) external view returns (ResolvedCrossChainOrder memory);
}
```

### IDestinationSettler Interface

```solidity
interface IDestinationSettler {
    function fill(
        bytes32 orderId,
        bytes calldata originData,
        bytes calldata fillerData
    ) external payable;
}
```

## Struct Mappings

### OnchainCrossChainOrder → OrderParams

ERC-7683's `OnchainCrossChainOrder` is an envelope containing opaque `orderData`:

| ERC-7683 Field | Maps To |
|----------------|---------|
| `fillDeadline` | `OrderParams.fillDeadline` |
| `orderDataType` | `keccak256("OrderParams(...)")` typehash |
| `orderData` | `abi.encode(OrderParams)` |

The wrapper decodes `orderData` into our `OrderParams` struct and overrides `sender = msg.sender` (the user calling the wrapper's `open()`). The wrapper itself becomes the OrderBook's "funder" (the account that pays input tokens and owns the nonce), while the user is recorded as the order's `sender` (owner) with cancellation rights and refund destination. See [Funder vs Sender](#funder-vs-sender) below.

### GaslessCrossChainOrder → GaslessOrderParams

| ERC-7683 Field | OrderBook Field | Notes |
|----------------|-----------------|-------|
| `originSettler` | (derived) | Must equal wrapper address |
| `user` | `sender` | Order owner |
| `nonce` | `nonce` | Cast uint256 → uint64 |
| `originChainId` | `originChainId` | Cast uint256 → uint32 |
| `openDeadline` | (validated) | Wrapper validates, not stored |
| `fillDeadline` | `fillDeadline` | Direct mapping |
| `orderDataType` | (derived) | Typehash for GaslessOrderData |
| `orderData` | (decoded) | Contains dest chain, tokens, amounts |

### GaslessOrderData (inside orderData)

```solidity
struct GaslessOrderData {
    uint32 destChainId;
    address tokenIn;
    bytes32 tokenOut;
    uint128 amountIn;
    uint128 amountOut;
    bytes32 recipient;
    bytes32 solver;
}
```

### ResolvedCrossChainOrder Construction

For our single-leg orders, the resolved order contains:

| Field | Value |
|-------|-------|
| `user` | `orderParams.sender` |
| `originChainId` | `block.chainid` |
| `openDeadline` | `type(uint32).max` for onchain orders |
| `fillDeadline` | `orderParams.fillDeadline` |
| `orderId` | Computed via `orderBook.getOrderId()` |
| `maxSpent[0]` | Filler's obligation (tokenOut, amountOut, recipient, destChainId) |
| `minReceived[0]` | Filler's reward (tokenIn, amountIn, filler, originChainId) |
| `fillInstructions[0]` | (destChainId, destSettler, abi.encode(OrderData)) |

### fill() Parameter Mapping

| ERC-7683 | OrderBook | Notes |
|----------|-----------|-------|
| `orderId` | `orderId_` | Direct mapping |
| `originData` | `orderData_` | `abi.decode(originData, (OrderData))` |
| `fillerData` | `fillParams_, bridgeAdapter_, bridgeAdapterArgs_` | Decode struct |

**fillerData encoding:**
```solidity
fillerData = abi.encode(FillParams, bridgeAdapter, bridgeAdapterArgs)
```

## Wrapper Implementation

### Contract Structure

```solidity
contract ERC7683Wrapper is IOriginSettler, IDestinationSettler {
    IOrderBook public immutable orderBook;

    bytes32 public constant ORDER_PARAMS_TYPEHASH = keccak256(
        "OrderParams(uint32 destChainId,uint32 fillDeadline,address tokenIn,"
        "bytes32 tokenOut,uint128 amountIn,uint128 amountOut,bytes32 recipient,"
        "bytes32 solver,address sender)"
    );

    bytes32 public constant GASLESS_ORDER_DATA_TYPEHASH = keccak256(
        "GaslessOrderData(uint32 destChainId,address tokenIn,bytes32 tokenOut,"
        "uint128 amountIn,uint128 amountOut,bytes32 recipient,bytes32 solver)"
    );

    constructor(address orderBook_) {
        orderBook = IOrderBook(orderBook_);
    }
}
```

### open() Implementation

```solidity
function open(OnchainCrossChainOrder calldata order) external override {
    require(order.orderDataType == ORDER_PARAMS_TYPEHASH, "Invalid orderDataType");

    IOrderBook.OrderParams memory params = abi.decode(order.orderData, (IOrderBook.OrderParams));

    // The user (msg.sender of this wrapper call) becomes the order's `sender` (owner).
    // The wrapper itself will be the OrderBook's funder (pays tokens, owns the nonce).
    params.sender = msg.sender;

    // Transfer tokens: user → wrapper → OrderBook
    // We cannot use openOrderWithPermit here: it requires msg.sender == orderParams.sender,
    // but our msg.sender to OrderBook will be the wrapper, not the user.
    IERC20(params.tokenIn).safeTransferFrom(msg.sender, address(this), params.amountIn);
    IERC20(params.tokenIn).approve(address(orderBook), params.amountIn);

    // Open order (tokens pulled from wrapper, ownership credited to the user via params.sender)
    bytes32 orderId = orderBook.openOrder(params);

    // Emit ERC-7683 event
    emit Open(orderId, _resolveOnchain(order, msg.sender, orderId));
}
```

### openFor() Implementation

```solidity
function openFor(
    GaslessCrossChainOrder calldata order,
    bytes calldata signature,
    bytes calldata /* originFillerData */
) external override {
    require(order.originSettler == address(this), "Invalid settler");
    require(block.timestamp <= order.openDeadline, "Open deadline passed");
    require(order.orderDataType == GASLESS_ORDER_DATA_TYPEHASH, "Invalid orderDataType");

    // Convert to GaslessOrderParams
    IOrderBook.GaslessOrderParams memory params = _toGaslessOrderParams(order);

    // Forward to OrderBook's gasless flow (signature validated there)
    bytes32 orderId = orderBook.openOrderFor(params, signature);

    emit Open(orderId, _resolveGasless(order, orderId));
}
```

### fill() Implementation

```solidity
function fill(
    bytes32 orderId,
    bytes calldata originData,
    bytes calldata fillerData
) external payable override {
    IOrderBook.OrderData memory orderData = abi.decode(originData, (IOrderBook.OrderData));
    (
        IOrderBook.FillParams memory fillParams,
        address bridgeAdapter,
        bytes memory bridgeAdapterArgs
    ) = abi.decode(fillerData, (IOrderBook.FillParams, address, bytes));

    // Transfer output tokens: filler → wrapper → recipient (via OrderBook)
    address tokenOut = address(uint160(uint256(orderData.tokenOut)));
    IERC20(tokenOut).safeTransferFrom(msg.sender, address(this), fillParams.amountOutToFill);
    IERC20(tokenOut).approve(address(orderBook), fillParams.amountOutToFill);

    orderBook.fillOrder{value: msg.value}(
        orderId,
        orderData,
        fillParams,
        bridgeAdapter,
        bridgeAdapterArgs
    );
}
```

### resolve() and resolveFor() Implementation

```solidity
function resolve(OnchainCrossChainOrder calldata order)
    external view override returns (ResolvedCrossChainOrder memory)
{
    IOrderBook.OrderParams memory params = abi.decode(order.orderData, (IOrderBook.OrderParams));
    // OrderData hash binds (sender, funder). At open() time the wrapper is the funder,
    // so resolve() must use address(this) — NOT msg.sender, which here is the caller of resolve().
    bytes32 orderId = _computeOrderId(params, msg.sender /* sender */, address(this) /* funder */);
    return _resolveOnchain(order, msg.sender, orderId);
}

function resolveFor(
    GaslessCrossChainOrder calldata order,
    bytes calldata /* originFillerData */
) external view override returns (ResolvedCrossChainOrder memory)
{
    bytes32 orderId = _computeGaslessOrderId(order);
    return _resolveGasless(order, orderId);
}
```

## Funder vs Sender

The OrderBook distinguishes two address roles on every open:

- **`sender`** (in `OrderParams.sender`, recorded as `Order.sender`): the order owner. Holds cancellation rights and is the refund destination.
- **`funder`** (`msg.sender` of the `openOrder` call): pays the input tokens, owns the per-funder nonce counter (`funderNonces[funder]`), and is included in the OrderData hash.

This wrapper uses that split: the user is the `sender`; the wrapper is the `funder`. Implications:

- The wrapper's per-funder nonce (`orderBook.getFunderNonce(address(this))`) advances on every onchain open.
- `resolve()` must compute the order ID using the wrapper as funder (see implementation above).
- The wrapper cannot use `openOrderWithPermit`: that entrypoint reverts with `InvalidSender` when `msg.sender != orderParams.sender`. The user must approve the wrapper directly (or use Permit2 / a deposit step).

## Token Flow

### open() Flow

1. User approves tokens to **wrapper**
2. User calls `wrapper.open(order)`
3. Wrapper pulls tokens from user via `transferFrom`
4. Wrapper approves tokens to OrderBook
5. Wrapper calls `orderBook.openOrder(params)` where `params.sender = user`
6. OrderBook pulls tokens from wrapper (the funder), records user as the order's sender (owner)

### openFor() Flow (Gasless)

1. User signs EIP-712 message approving order
2. User approves tokens to **OrderBook** directly
3. Relayer calls `wrapper.openFor(order, signature, "")`
4. Wrapper validates openDeadline
5. Wrapper calls `orderBook.openOrderFor(params, signature)`
6. OrderBook validates signature, pulls tokens from user, records user as owner

### fill() Flow

1. Filler approves output tokens to **wrapper**
2. Filler calls `wrapper.fill(orderId, originData, fillerData)`
3. Wrapper pulls output tokens from filler
4. Wrapper approves output tokens to OrderBook
5. Wrapper calls `orderBook.fillOrder(...)` with decoded params

## Type Conversions

The wrapper performs safe casts for type mismatches:

| ERC-7683 Type | OrderBook Type | Cast |
|---------------|----------------|------|
| `uint256 nonce` | `uint64 nonce` | `uint64(order.nonce)` |
| `uint256 originChainId` | `uint32 originChainId` | `uint32(order.originChainId)` |

These casts are safe because our OrderBook values will always fit within the smaller types. The wrapper should reject (rather than silently truncate) inputs whose high bits are non-zero, to satisfy ERC-7683 round-tripping.

Note: the OrderBook `OrderData` struct also carries a `bytes32 funder` field (set to the wrapper address for orders opened via `open()`, and to the signer for orders opened via `openFor()`). This field is part of the order ID hash and must be supplied correctly to `_computeOrderId` in `resolve()` / `resolveFor()`.

## Open Deadline Handling

ERC-7683's `openDeadline` specifies when an order must be opened by. Our OrderBook doesn't have this concept. The wrapper:

1. **For `openFor()`**: Validates `block.timestamp <= order.openDeadline` before forwarding
2. **For `open()`**: Uses `type(uint32).max` as openDeadline in `ResolvedCrossChainOrder` (no deadline for direct orders)

## Event Emission

The wrapper emits the ERC-7683 `Open` event with the resolved order:

```solidity
event Open(bytes32 indexed orderId, ResolvedCrossChainOrder resolvedOrder);
```

This is emitted after the underlying `OrderBook.openOrder()` or `OrderBook.openOrderFor()` succeeds.

## Testing Plan

1. **Unit tests**
   - `open()` correctly decodes orderData and sets sender
   - `openFor()` validates openDeadline and forwards signature
   - `resolve()` and `resolveFor()` return correct ResolvedCrossChainOrder
   - `fill()` correctly decodes and forwards parameters

2. **Integration tests**
   - Full flow: open via wrapper → fill via wrapper → settlement
   - Cross-chain flow with Portal messaging
   - Token approvals work correctly through wrapper

3. **Edge cases**
   - Invalid orderDataType reverts
   - Expired openDeadline reverts
   - Type overflow handling (should not occur with our values)
   - `resolve()` returns the same order ID that `open()` actually produces (regression for the funder-in-hash binding)
   - Wrapper-as-funder nonce advances correctly when the same user opens multiple orders concurrently
   - Direct call to `orderBook.openOrderWithPermit` with the wrapper's signature would revert (`InvalidSender`); confirm we never invoke this path

## Future Considerations

1. **Multi-leg orders**: ERC-7683 supports multi-leg orders via `Output[]` arrays. Our single-leg design uses arrays of length 1.

2. **originFillerData**: Currently unused. Could be used for filler-specific routing hints.

3. **Gas optimization**: The double transfer (user → wrapper → OrderBook) adds gas overhead. Consider using permit2 or direct approvals.
