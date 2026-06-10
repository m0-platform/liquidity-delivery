The [M0 Liquidity Delivery Network](https://www.notion.so/M0-Liquidity-Delivery-Network-213858df176a80eca9dfc48969a864fe?pvs=21) defines an overall vision of liquidity delivery for the M0 ecosystem. A key part of this is an onchain limit order system that can be used to provide liquidity between M, it’s extensions, and other assets at a fixed price on a flexible timeline. The [M0 Liquidity Delivery Requirements ](https://www.notion.so/M0-Liquidity-Delivery-Requirements-26c858df176a807b9009e002e212da0a?pvs=21) defines the requirements of this system and its relationship to other projects that are required to realize the vision. This document defines a design and engineering specification to implement the Intent / Limit Order system required from these documents.

## Why is this important?

Our general stack is a commodity that others are copying. Liquidity is the moat. We need to win in this feature and build the best solution we can.

# Requirements

- Must operate on all chains where M0 liquidity needs to be delivered
- Support partial fills to allow solvers to handle large orders incrementally
- Support samechain and crosschain orders
- User should be able to define a price for their order. Solvers can set prices they will fill at. However, the price of the order should be fixed and known.
- Require no trust assumptions on solvers
- Solvers must maintain inventory to fill orders but may partially fill and rebalance as needed (no pre-minting)
- Allow users to specify an exclusive solver for an order (i.e. if they want a known counterparty)

# Components

## OrderBook Contract / Program

Contract that allows the submission and fulfillment of same-chain and cross-chain intent orders. In the case of cross-chain orders, fills and cancels are reported from the destination chain back to the origin chain via M0's Portal V2 cross-chain messaging protocol. The OrderBook contracts use the chain IDs that the Portal uses to reference each chain, which are converted to the underlying messaging-network IDs by the Portal's Bridge Adapters. The Portal supports multiple bridge adapters per route, so users can optionally specify which adapter to use when sending a fill or cancel report. EVM and SVM versions are both implemented to ensure coverage across the chains where M0 extensions live.

General intent protocols typically seek to obtain the best price for users via a competition between solvers to fill orders. We desire fixed prices (aka limit orders) in our protocol so this complexity is not required. Users are able to specify an exclusive solver for their order (likely inserted for them based on the interface they interact with) to avoid solvers having to race to fill orders in most cases. Three solver-selection scenarios are supported:

1. **Exclusive solver** — User specifies a solver address; only that solver can fill the order. Useful for known counterparties.
2. **Open, single-fill** — User specifies the zero solver and any solver can race to fill the full order.
3. **Open, multi-fill** — User specifies the zero solver and multiple solvers can each take partial fills, useful for large orders that exceed any one solver's inventory.

A key requirement that is not present in most intent protocols is supporting partial fills in order to allow a solver to rebalance several times, if necessary, to fill large orders. We’re optimizing for order size and volume over a period of time rather than atomic orders.

### Architecture

Here is a high-level overview of the architecture and core user flow of submitting and filling orders between chains.

https://link.excalidraw.com/readonly/2LbJFVLBvmTNU5u88CFq

The core user flow for a cross-chain order is:

1. User submits an order to the OrderBook contract on any supported chain, transferring the `amountIn` of the `tokenIn` to it. This chain is the origin chain. The user may specify a solver to fill the order, or, if none is provided, any solver can fill the order. Additionally, they specify a `fillDeadline` that the order must be filled by or it expires.
2. The (if exclusive) or a (if open) solver fills the order on the destination chain within the fill deadline specified by the user. When the order is filled, the recipient receives the `amountOut` of the `tokenOut` on the destination chain. Orders can be filled incrementally to allow the solver to replenish their inventory for large orders. If the order is not filled or only partially filled before the fill deadline, the recipient (or, after the deadline, anyone) can cancel the order on the destination chain to make any remaining `amountIn` of `tokenIn` refundable on the origin chain.
3. When the order is filled, the destination chain sends a `FillReport` cross-chain message via the Portal back to the origin chain. When that message is delivered, the OrderBook on the origin chain releases the corresponding amount of `tokenIn` to the solver-specified `originRecipient`, completing the order.

The flow is the same for a same-chain order without the added complexity of needing to report the fill or cancel from the destination chain — the contract releases / refunds funds atomically inside the same transaction.

Cancellation follows the same shape as filling: cancels are initiated on the destination chain and, for cross-chain orders, a `CancelReport` is sent back through the Portal to release the unfilled remainder as a refund. There is no "request cancel + wait" flow on the origin chain — the cancel happens on the destination chain so that any in-flight fills race against the cancel correctly.

More details about each user flow can be found on this [excalidraw presentation](https://link.excalidraw.com/p/readonly/tRUg0gOL4P2DOLCcq09D) (there are multiple slides).

### Conventions

- Token amounts are 128-bit unsigned integers
- Addresses on other chains are stored as 32 bytes to support non-EVM chains (e.g. Solana). On the origin chain only, the input token and order sender are also stored in their native 20-byte form for convenience.
- Timestamp values are stored as 64-bit unsigned integers in `OrderData` (the cross-chain payload) so they remain well-defined across chains. The locally-stored `Order` struct on EVM uses 32-bit timestamps for slot packing.

### Data Structures (EVM)

```solidity
enum OrderStatus {
    DoesNotExist,
    Created,
    Cancelled,
    Completed
}

// Parameters supplied by the user when opening an order onchain
struct OrderParams {
    uint32 destChainId;
    uint32 fillDeadline;  // timestamp by which the order must be filled
    address tokenIn;
    bytes32 tokenOut;     // 32 bytes to accommodate non-EVM destinations
    uint128 amountIn;
    uint128 amountOut;
    bytes32 recipient;
    bytes32 solver;       // zero == any solver may fill
    address sender;       // owner of the order (refund/cancel rights); tokens are
                          // pulled from msg.sender, allowing a wrapper contract
                          // to fund an order on behalf of `sender`
}

// Complete data about an order, stored only on the origin chain
struct Order {
    OrderStatus status;
    uint16 version;
    address sender;
    uint64 nonce;
    uint32 destChainId;
    uint32 createdAt;
    uint32 fillDeadline;
    address tokenIn;
    bytes32 tokenOut;
    uint128 amountIn;
    uint128 amountOut;
    bytes32 recipient;
    bytes32 solver;
}

// Cross-chain payload used to compute the order ID and to fill/cancel
// the order on the destination chain. Addresses are all 32 bytes.
struct OrderData {
    uint16 version;
    bytes32 sender;
    uint64 nonce;
    uint32 originChainId;
    uint32 destChainId;
    uint64 createdAt;
    uint64 fillDeadline;
    bytes32 tokenIn;
    bytes32 tokenOut;
    uint128 amountIn;
    uint128 amountOut;
    bytes32 recipient;
    bytes32 solver;
}

// Order ID — a unique, cross-chain identifier
// Computed identically on EVM and SVM (see svm/programs/order_book/src/state/orders.rs
// for the cross-chain compatibility test)
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

// Fill / refund progress, stored on both origin and destination chains
struct FilledAmounts {
    uint128 amountInRefunded;  // released back to the order sender
    uint128 amountInReleased;  // released to solver(s) after fill
    uint128 amountOutFilled;   // amount of tokenOut delivered to the recipient
}

// Cross-chain message reporting a fill back to the origin chain
struct FillReport {
    bytes32 orderId;
    uint128 amountInToRelease;
    uint128 amountOutFilled;
    bytes32 originRecipient;   // who receives the released tokenIn on the origin chain
    bytes32 tokenIn;           // included so non-EVM origins can resolve the token
}

// Cross-chain message reporting a cancel back to the origin chain
struct CancelReport {
    bytes32 orderId;
    bytes32 orderSender;
    bytes32 tokenIn;
    uint128 amountInToRefund;
}

// Parameters supplied by the solver when filling
struct FillParams {
    uint128 amountOutToFill;
    bytes32 originRecipient;   // recipient of released tokenIn on the origin chain
    bytes32 refundAddress;     // optional Portal bridge-fee refund address; zero == msg.sender
}
```

The contract version is exposed as `uint16 public constant VERSION = 1`.

### State (EVM)

The OrderBook contract is upgradeable and uses the ERC-7201 storage-namespace pattern:

```solidity
struct OrderBookStorageStruct {
    // Destinations explicitly enabled by an admin. The current chain is always
    // considered supported (for same-chain orders) without an entry here.
    mapping(uint32 destChainId => bool isSupported) supportedDestinations;
    // Full data for orders that originated on this chain
    mapping(bytes32 orderId => Order) orders;
    // Fill / refund amounts for both origin and destination orders
    mapping(bytes32 orderId => FilledAmounts) filledAmounts;
    // Per-sender nonce used to ensure unique order IDs
    mapping(address sender => uint64 nonce) senderNonces;
}

// The portal handling cross-chain messaging is set immutably at deploy time.
address public immutable portal;

// Roles
bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
// DEFAULT_ADMIN_ROLE manages destinations and role grants.
```

### Events

```solidity
event OrderOpened(
    bytes32 orderId,
    address funder,
    address indexed sender,
    address tokenIn,
    uint128 amountIn,
    uint32 indexed destChainId,
    bytes32 tokenOut,
    uint128 amountOut,
    bytes32 indexed solver,
    uint32 fillDeadline
);
event OrderFilled(
    bytes32 indexed orderId,
    address indexed solver,
    uint128 amountInToRelease,
    uint128 amountOutFilled,
    bytes32 indexed messageId  // zero for same-chain fills
);
event OrderCancelled(bytes32 indexed orderId, bytes32 indexed messageId);
event OrderCompleted(bytes32 orderId);
event FillReported(
    bytes32 indexed orderId,
    address indexed originRecipient,
    uint128 amountInToRelease,
    uint128 amountOutFilled
);
event CancelReported(bytes32 indexed orderId);
event RefundClaimed(bytes32 indexed orderId, address indexed sender, uint128 amountInRefunded);
event DestinationSupportUpdated(uint32 indexed destChainId, bool isSupported);
```

### User Actions

**Submitting Orders Onchain**

```solidity
function openOrder(OrderParams calldata orderParams) external returns (bytes32 orderId);

// Variants accepting an EIP-2612 permit so the user can approve and open in one tx
function openOrderWithPermit(
    OrderParams calldata orderParams,
    uint256 deadline,
    uint8 v, bytes32 r, bytes32 s
) external returns (bytes32 orderId);

function openOrderWithPermit(
    OrderParams calldata orderParams,
    uint256 deadline,
    bytes memory permitSignature
) external returns (bytes32 orderId);
```

`openOrder` validates that:

- `fillDeadline >= block.timestamp`
- `amountIn > 0`, `amountOut > 0`
- `recipient != 0` and `recipient != solver`
- `sender != 0`
- For same-chain orders, `tokenOut != tokenIn`
- The destination chain is supported (the current chain is always allowed for same-chain orders)

It then increments `sender`'s nonce, computes the order ID, stores the `Order`, and pulls `amountIn` of `tokenIn` from `msg.sender` (the funder) via `safeTransferExactFrom` (which guards against fee-on-transfer mismatches). The `funder` (`msg.sender`) and the order's `sender` (the owner that holds cancel/refund rights) may differ — this allows a wrapper contract to fund an order on behalf of an end user without taking custody of the order itself. The `OrderOpened` event includes both `funder` and `sender` so indexers can attribute the order correctly.

**Cancelling Orders**

Cancellation has been simplified to a single onchain action that always happens on the **destination** chain. The original "request cancel + wait finality buffer + claim refund" three-step flow is no longer used.

```solidity
// Recipient (or sender, for same-chain orders) before the deadline.
// Anyone after the deadline (permissionless refunds).
function cancelOrder(bytes32 orderId, OrderData calldata orderData)
    external payable returns (bytes32 messageId);

// Overloads accepting bridgeAdapter and bridgeAdapterArgs direct the resulting
// CancelReport message through a specific Portal V2 bridge.
```

Authorization rules:

- **Same-chain order, before deadline:** order's `sender` or `recipient` may cancel.
- **Cross-chain order, before deadline:** only the order's `recipient` may cancel (the `sender` lives on a different chain and cannot necessarily call this contract).
- **After deadline (any order):** anyone may cancel.

Behavior:

- **Same-chain (origin == dest):** the unfilled remainder of `amountIn` is transferred immediately back to the order sender; `msg.value` must be zero; status becomes `Cancelled`.
- **Cross-chain (origin != dest):** the destination-chain order's status is set to `Cancelled`, and a `CancelReport` is sent via the Portal back to the origin chain. `msg.value` is forwarded to the Portal as the bridge fee; the caller becomes the bridge fee `refundAddress`. On the origin chain, `reportCancel` releases the remaining `amountIn` to the original sender.

Because cross-chain messages can arrive out of order, the origin chain's `reportFill` accepts fills against orders in either `Created` or `Cancelled` status, and the running invariant `amountInReleased + amountInRefunded <= amountIn` prevents any over-distribution.

### Solver Actions

**Fill Order (on Destination Chain)**

```solidity
function fillOrder(
    bytes32 orderId,
    OrderData calldata orderData,
    FillParams calldata fillerParams
) external payable returns (bytes32 messageId);

// Overloads to direct the FillReport through a specific Portal V2 bridge adapter
function fillOrder(
    bytes32 orderId,
    OrderData calldata orderData,
    FillParams calldata fillerParams,
    bytes calldata bridgeAdapterArgs
) external payable returns (bytes32 messageId);

function fillOrder(
    bytes32 orderId,
    OrderData calldata orderData,
    FillParams calldata fillerParams,
    address bridgeAdapter,
    bytes calldata bridgeAdapterArgs
) external payable returns (bytes32 messageId);
```

`fillOrder` validates that the supplied `orderData` hashes to `orderId`, that the destination chain is the current chain, that the deadline has not passed, that the version matches, and (if the order specifies an exclusive solver) that `msg.sender` is that solver. The actual filled amount is the lesser of `fillerParams.amountOutToFill` and the remaining unfilled amount; the contract then transfers `amountOutToFill` of `tokenOut` from the solver to the recipient.

- For same-chain orders, the corresponding amount of `tokenIn` is released to `fillerParams.originRecipient` immediately and `msg.value` must be zero.
- For cross-chain orders, the contract sends a `FillReport` via the Portal back to the origin chain. `msg.value` is forwarded as the Portal bridge fee; `fillerParams.refundAddress` (or `msg.sender` if zero) receives any over-payment refund from the bridge.

### Receiving Crosschain Reports

```solidity
// Portal receives the message, decodes it, and calls these on the origin chain
function reportFill(uint32 sourceChainId, FillReport calldata report) external; // onlyPortal
function reportCancel(uint32 sourceChainId, CancelReport calldata report) external; // onlyPortal
```

`reportFill` accepts fills against orders that are `Created` or `Cancelled` (to allow in-flight fills that race a cancel) and validates that the cumulative `amountInReleased + amountInRefunded <= amountIn` and that `amountOutFilled <= amountOut`. It then releases the reported `amountInToRelease` to `originRecipient`. `reportCancel` is only valid against `Created` orders, sets the status to `Cancelled`, and refunds the sender.

### Admin Actions

```solidity
function setDestinationSupported(uint32 destChainId, bool isSupported) external; // DEFAULT_ADMIN_ROLE
function pause() external;   // PAUSER_ROLE
function unpause() external; // PAUSER_ROLE
```

The pause covers external order actions that would create new in-flight state (`openOrder*`, `cancelOrder*`, `fillOrder`) but **does not** affect inbound `reportFill` / `reportCancel`. This lets us pause the contract, drain in-flight cross-chain messages, and execute an upgrade gracefully (a behavior added per audit findings).

### View Helpers

```solidity
function getOrderId(OrderData calldata orderData) external pure returns (bytes32);
function getOrder(bytes32 orderId) external view returns (Order memory);
function getOrderData(bytes32 orderId) external view returns (OrderData memory);
function getFilledAmounts(bytes32 orderId) external view returns (FilledAmounts memory);
function getSenderNonce(address sender) external view returns (uint64);
function isDestinationSupported(uint32 destChainId) external view returns (bool);
```

### Deployment Topology (EVM)

The implementation is deployed behind an OpenZeppelin `TransparentUpgradeableProxy`, with the proxy address derived via CREATE3 to give a deterministic address across chains. The `ProxyAdmin` is owned by the configured admin address. The `portal` reference is set as an `immutable` in the constructor, so re-pointing to a new portal requires a full implementation upgrade.

### SVM Implementation Notes

The Solana program (`svm/programs/order_book`) implements the same protocol with conventions appropriate to Anchor / SPL-token environments:

- **Order accounts.** Each order is its own PDA seeded by `["order", orderId]`. Two distinct shapes share that seed:
  - `Order<NativeOrder>` — the order originated on this chain (full data, status, and filled amounts in one account).
  - `Order<ForeignOrder>` — the order originated on another chain and is being filled / cancelled here (status + filled amounts only). A foreign order PDA is created lazily on first fill or cancel.
- **Nonce account.** Per-sender nonces are tracked in `Nonce` PDAs seeded by `["nonce", sender]`.
- **Global account.** A single `OrderBookGlobal` PDA holds the chain ID, the configured `portal_authority`, the admin (and pending `new_admin` for two-step transfers), and the paused flag.
- **Destination accounts.** Supported destination chains are represented by `Destination` PDAs seeded by `["destination", destChainId]`.
- **`createdAt` window.** Because Solana clients can't deterministically know which slot an instruction will land in, the user supplies `createdAt` and the program accepts any value within `[now, now + 300s]`. This lets the order PDA be precomputed offchain.
- **Order ID compatibility.** `OrderData` is encoded with big-endian numeric fields and 32-byte addresses and hashed with keccak256, identical to the EVM packed encoding. A unit test in `state/orders.rs` builds the EVM `OrderData` via `alloy::sol!` against the compiled `IOrderBook.json` ABI and asserts that both implementations produce the same order ID.
- **Fills/cancels.** `fill_native_order` / `fill_foreign_order` and `cancel_native_order` / `cancel_foreign_order` mirror the EVM flow, with cross-chain reports going out via CPI into the Portal program. `report_order_fill` and `report_order_cancel` are gated by a `portal_authority` signer recorded on the global account.
- **Pause.** Same semantics as EVM: open / fill / cancel respect the `paused` flag; report instructions do not, so in-flight messages still settle while paused.
- **Token-account cleanup.** A dedicated `close_order_token_account` instruction reclaims rent from the order's token-in ATA after the order is fully settled.

## Indexer

Offchain service to ingest blockchain events from the LDN contracts and store them persistently. Measures need to be taken to ensure events are not missed, especially on high-throughput chains like Solana. Subgraphs are the preferred indexer + persistence solution on EVM chains. For SVM chains, we use a substream to index the events and store them in a database since there were issues with using a subgraph. It would be nice to have a consistent approach across both environments.

## Solver Bot

Offchain service that consumes order events provided by indexers and acts based on them to fill user orders, rebalancing its funds from available liquidity sources if required. It may be interesting to integrate the solver with a Fireblocks vault and use a callback handler to verify transactions it makes as a safety measure. Further down the road, it may even be interesting to consider allowing a bot (operated by an approved Minter) to mint M using onchain collateral and API queries to validator endpoints.

## Limitations

- Solver compensation is not handled by the protocol onchain. However, solver volume is tracked via events and can be aggregated offchain. It is likely possible to handle distributions onchain via a merkle-style distribution, but I leave this for future consideration.
- This design is optimized to accommodate large orders that require multiple, partial fills to complete. It is not as efficient as other designs for filling many small orders, especially since we do not batch fill reports for cross-chain orders. Batching could be added later as an optimization without changing the user-facing protocol.
