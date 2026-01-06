// SPDX-License-Identifier: GPL-3.0
pragma solidity >=0.8;

interface IOrderBook {
    /* ========== Events ========== */

    /**
     * @notice Emitted when a new order is opened
     * @dev This event is emitted on the origin chain
     * @param orderId The ID of the order
     * @param sender The address that provided the funds on the origin (this) chain
     * @param tokenIn The address of the input token on the origin (this) chain
     * @param amountIn The amount of input token provided
     * @param destChainId The internal chain ID where the order will be filled
     * @param tokenOut The address of the output token on the destination chain
     * @param amountOut The amount of output token expected
     * @param solver The address of the solver that will fill the order, or zero address if any solver can fill
     */
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

    /**
     * @notice Emitted when an order is filled
     * @dev This event is emitted on the destination chain
     * @param orderId The ID of the order being filled
     * @param solver The address of the solver that filled the order
     * @param amountOutFilled The amount of output token that was filled
     * @param amountInToRelease The amount of input token they will receive on the origin chain
     */
    event OrderFilled(
        bytes32 indexed orderId,
        address indexed solver,
        uint128 amountInToRelease,
        uint128 amountOutFilled
    );

    /**
     * @notice Emitted when a refund is claimed for an order
     * @dev This event is emitted on the origin chain
     * @param orderId The ID of the order being refunded
     * @param sender The address that the refund is sent to
     * @param amountInRefunded The amount of input token that was refunded
     */
    event RefundClaimed(bytes32 indexed orderId, address indexed sender, uint128 amountInRefunded);

    /**
     * @notice Emitted when an order is completed (fully filled)
     * @dev This event is emitted on the destination chain
     * @param orderId The ID of the completed order
     */
    event OrderCompleted(bytes32 orderId);

    /**
     * @notice Emitted when an order is cancelled
     * @dev This event is emitted on the destination chain
     * @param orderId The ID of the cancelled order
     */
    event OrderCancelled(bytes32 indexed orderId);

    /**
     * @notice Emitted when the support for a destination chain is updated
     * @dev This event is emitted on the origin chain
     * @param destChainId The internal chain ID of the destination chain
     * @param isSupported Whether orders can be created with this chain as the destination
     */
    event DestinationSupportUpdated(uint32 indexed destChainId, bool isSupported);

    /* ========== Errors ========== */
    error AmountInZero();
    error AmountOutZero();
    error FillAmountZero();
    error FinalityPending();
    error InvalidDeadline();
    error InvalidDestinationChain();
    error InvalidFinalityBuffer();
    error InvalidMsgValue();
    error InvalidNonce();
    error InvalidOrderStatus();
    error InvalidOrderVersion();
    error InvalidOriginChain();
    error InvalidRecipient();
    error InvalidSolver();
    error InvalidReport();
    error InvalidTimestamp();
    error NotAuthorized();
    error OrderExpired();
    error OrderAlreadyExists();
    error OrderAlreadyFilled();
    error OrderIdMismatch();

    /* ========== Structs and Enums ========== */

    /**
     * @notice Parameters required to open an order onchain
     * @dev Addresses on the destination chain are stored as bytes32 to support non-EVM chains as destinations
     * @param destChainId Destination chain ID where the order is to be filled
     * @param fillDeadline Timestamp by which the order must be filled on the destination chain
     * @param tokenIn Address of the input token on the origin (this) chain
     * @param tokenOut Address of the output token on the destination chain
     * @param amountIn Amount of input token provided
     * @param amountOut Amount of output token expected
     * @param recipient Address to receive the funds on the destination chain
     * @param solver Address of the solver that will fill the order, or zero address if any solver can fill
     */
    struct OrderParams {
        uint32 destChainId;
        uint32 fillDeadline;
        address tokenIn;
        bytes32 tokenOut;
        uint128 amountIn;
        uint128 amountOut;
        bytes32 recipient;
        bytes32 solver;
    }

    /**
     * @notice Parameters required to open a gasless order onchain
     * @dev Addresses on the destination chain are stored as bytes32 to support non-EVM chains as destinations
     * @dev This payload is hashed and included as the internal digest of the
     *      EIP-712 payload required for gasless order submission
     * @param version Version of the contract the order is created for
     * @param sender Address that provided the funds on the origin chain, must sign the payload
     * @param nonce Unique identifier for the order, must match the sender's next nonce on this chain
     * @param originChainId internal chain ID where the order is created (must be this chain)
     * @param destChainId internal chain ID where the order is to be filled
     * @param fillDeadline Timestamp by which the order must be filled on the destination chain
     * @param tokenIn Address of the input token on the origin chain
     * @param tokenOut Address of the output token on the destination chain
     * @param amountIn Amount of input token provided
     * @param amountOut Amount of output token expected
     * @param recipient Address to receive the funds on the destination chain
     * @param solver Address of the solver that will fill the order, or zero address if any solver can fill
     */
    struct GaslessOrderParams {
        uint16 version;
        address sender;
        uint64 nonce;
        uint32 originChainId;
        uint32 destChainId;
        uint32 fillDeadline;
        address tokenIn;
        bytes32 tokenOut;
        uint128 amountIn;
        uint128 amountOut;
        bytes32 recipient;
        bytes32 solver;
    }

    /**
     * @notice Possible order statuses
     */
    enum OrderStatus {
        DoesNotExist,
        Created,
        Cancelled,
        Completed
    }

    /**
     * @notice Complete data about an order originated on this chain
     * @dev Addresses on the destination chain are stored as bytes32 to support non-EVM chains
     * @param status Current status of the order
     * @param version Version of the contract when the order was created
     * @param sender Address that provided the funds on the origin chain
     * @param nonce A counter tied to the sender to allow unique orders
     * @param destChainId Destination chain ID where the order is to be filled
     * @param createdAt Timestamp when the order was created
     * @param fillDeadline Timestamp by which the order must be filled on the destination chain
     * @param tokenIn Address of the input token on this chain
     * @param tokenOut Address of the output token on the destination chain
     * @param amountIn Amount of input token provided
     * @param amountOut Amount of output token expected
     * @param recipient Address to receive the funds on the destination chain
     * @param solver Address of the solver that will fill the order, or zero address if any approved solver can fill
     */
    struct Order {
        OrderStatus status; // slot 1: 1 +
        uint16 version; //             2 +
        address sender; //             20 +
        uint64 nonce; //               8 = 31 bytes
        uint32 destChainId; // slot 2: 4 +
        uint32 createdAt; //           4 +
        uint32 fillDeadline; //        4 +
        address tokenIn; //            20 = 32 bytes
        bytes32 tokenOut; //   slot 3
        uint128 amountIn; //   slot 4: 16 +
        uint128 amountOut; //          16 = 32 bytes
        bytes32 recipient; //  slot 5
        bytes32 solver; //     slot 6
    }

    /**
     * @notice Data required to identify and fill an order on a destination chain
     * @dev This struct is used to compute a unique order ID and to provide all necessary
     *      information to fill the order on the destination chain
     *      The order ID is computed as the keccak256 hash of the packed-encoding of this struct
     * @param version Version of the contract when the order was created
     * @param sender Address that provided the funds on the origin chain
     * @param nonce A counter tied to the sender to allow unique orders
     * @param originChainId internal chain ID where the order was created
     * @param destChainId Destination chain ID where the order is to be filled
     * @param createdAt Timestamp when the order was created
     * @param fillDeadline Timestamp by which the order must be filled on the destination chain
     * @param tokenIn Address of the input token on the origin chain
     * @param tokenOut Address of the output token on the destination chain
     * @param amountIn Amount of input token provided
     * @param amountOut Amount of output token expected on the destination chain
     * @param recipient Address to receive the funds on the destination chain
     * @param solver Address of the solver that will fill the order, or zero address if any approved solver can fill
     */
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

    /**
     * @notice Data reported from a destination chain back to the origin chain about a fill
     * @dev This struct is sent by the messenger contract to report fills that occurred
     *      on the destination chain back to the origin chain for processing
     * @param orderId The ID of the order that a fill is being reported for
     * @param amountInToRelease The amount of input token to release to the filler on the origin chain
     * @param amountOutFilled The amount of output token that was filled on the destination chain
     * @param originRecipient The address on the origin chain that should receive released funds
     * @param tokenIn The address of the input token on the origin chain
     *                This is included for non-EVM chains to provide a way to resolve the token
     */
    struct FillReport {
        bytes32 orderId;
        uint128 amountInToRelease;
        uint128 amountOutFilled;
        bytes32 originRecipient;
        bytes32 tokenIn;
    }

    /**
     * @notice Data reported from a destination chain back to the origin chain about a cancelled order
     * @dev This struct is sent by the messenger contract to report order cancellations and refunds
     *      that occurred on the destination chain back to the origin chain for processing
     * @param orderId The ID of the order that a cancellation is being reported for
     * @param orderSender The address on the origin chain that created the order
     * @param tokenIn The address of the input token on the origin chain
     * The last two are included for non-EVM chains to provide a way to resolve the sender and token
     */
    struct CancelReport {
        bytes32 orderId;
        bytes32 orderSender;
        bytes32 tokenIn;
    }

    /**
     * @notice Parameters supplied by the filler of an order
     * @dev This struct contains parameters that are specific to the filler
     *      and are not part of the original order data
     * @param amountOutToFill The amount of output token the filler is providing to fill
     * @param originRecipient The address on the origin chain that should receive released funds
     */
    struct FillParams {
        uint128 amountOutToFill;
        bytes32 originRecipient;
    }

    /**
     * @notice Data structure to track filled amounts for an order on the destination chain
     * @param amountOutFilled Amount of output token filled
     * @param amountInReleased Amount of input token released
     */
    struct FilledAmounts {
        uint128 amountInReleased;
        uint128 amountOutFilled;
    }

    /* ========== Creating Orders ========== */

    /**
     * @notice Opens an order
     * @dev Must be called by the user providing the input funds
     * @param orderParams_ order creation parameters (see OrderParams definition)
     * @return The unique ID of the opened order
     */
    function openOrder(OrderParams calldata orderParams_) external returns (bytes32);

    /**
     * @notice Opens an order with an EIP-2612 permit signature for token approval
     * @dev Must be called by the user providing the input funds
     * @param orderParams_ order creation parameters (see OrderParams definition)
     * @param deadline_ deadline for the permit signature
     * @param v_ v parameter of the permit signature
     * @param r_ r parameter of the permit signature
     * @param s_ s parameter of the permit signature
     * @return The unique ID of the opened order
     */
    function openOrderWithPermit(
        OrderParams calldata orderParams_,
        uint256 deadline_,
        uint8 v_,
        bytes32 r_,
        bytes32 s_
    ) external returns (bytes32);

    /**
     * @notice Opens an order with an EIP-2612 permit signature for token approval
     * @dev Must be called by the user providing the input funds
     * @param orderParams_ order creation parameters (see OrderParams definition)
     * @param deadline_ deadline for the permit signature
     * @param permitSignature_ packed encoding of the permit signature
     * @return The unique ID of the opened order
     */
    function openOrderWithPermit(
        OrderParams calldata orderParams_,
        uint256 deadline_,
        bytes memory permitSignature_
    ) external returns (bytes32);

    /**
     * @notice Opens a gasless order on behalf of a user
     * @dev More flexible method relying on an offchain signature to authorize order creation
     * @param orderParams_ gasless order creation parameters (see GaslessOrderParams definition)
     * @param orderSignature_ Order sender's signature of the EIP-712 payload
     *        containing the orderParams (see getGaslessOrderDigest)
     * @return The unique ID of the opened order
     */
    function openOrderFor(
        GaslessOrderParams calldata orderParams_,
        bytes calldata orderSignature_
    ) external returns (bytes32);

    /**
     * @notice Opens a gasless order on behalf of a user with an EIP-2612 permit signature for token approval
     * @dev More flexible method relying on an offchain signature to authorize order creation
     * @param orderParams_ gasless order creation parameters (see GaslessOrderParams definition)
     * @param orderSignature_ Order sender's signature of the EIP-712 payload
     *        containing the orderParams (see getGaslessOrderDigest)
     * @param deadline_ deadline for the permit signature
     * @param v_ v parameter of the permit signature
     * @param r_ r parameter of the permit signature
     * @param s_ s parameter of the permit signature
     * @return The unique ID of the opened order
     */
    function openOrderForWithPermit(
        GaslessOrderParams calldata orderParams_,
        bytes calldata orderSignature_,
        uint256 deadline_,
        uint8 v_,
        bytes32 r_,
        bytes32 s_
    ) external returns (bytes32);

    /**
     * @notice Opens a gasless order on behalf of a user with an EIP-2612 permit signature for token approval
     * @dev More flexible method relying on an offchain signature to authorize order creation
     * @param orderParams_ gasless order creation parameters (see GaslessOrderParams definition)
     * @param orderSignature_ Order sender's signature of the EIP-712 payload
     *        containing the orderParams (see getGaslessOrderDigest)
     * @param deadline_ deadline for the permit signature
     * @param permitSignature_ packed encoding of the permit signature
     * @return The unique ID of the opened order
     */
    function openOrderForWithPermit(
        GaslessOrderParams calldata orderParams_,
        bytes calldata orderSignature_,
        uint256 deadline_,
        bytes memory permitSignature_
    ) external returns (bytes32);

    /* ========== Refunding Orders ========== */

    /**
     * @notice Cancel an order
     * @dev Must be called by the order's recipient (or sender if same chain order) before fill deadline
     * @dev Can be called by anyone after the fill deadline (permissionless refunds)
     * @param orderId_ - ID of the order to cancel
     * @param orderData_ OrderData payload with all order information required to identify an order to be cancelled
     * @dev   The payable amount is forwarded to the underlying messenger contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function cancelOrder(bytes32 orderId_, OrderData calldata orderData_) external payable;

    /**
     * @notice Cancel an order with additional message data required by some crosschain messages
     * @dev Must be called by the order's recipient (or sender if same chain order) before fill deadline
     * @dev Can be called by anyone after the fill deadline (permissionless refunds)
     * @param orderId_ ID of the order to cancel
     * @param orderData_ OrderData payload with all order information required to identify an order to be cancelled
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @dev   The payable amount is forwarded to the underlying messenger contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function cancelOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata bridgeAdapterArgs_
    ) external payable;

    /**
     * @notice Cancel an order with additional message data required by some crosschain messages
     * @dev Must be called by the order's recipient (or sender if same chain order) before fill deadline
     * @dev Can be called by anyone after the fill deadline (permissionless refunds)
     * @param orderId_ ID of the order to cancel
     * @param orderData_ OrderData payload with all order information required to identify an order to be cancelled
     * @param bridgeAdapter_ Address of the bridge adapter to use for crosschain messages (must be supported by Portal V2)
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @dev   The payable amount is forwarded to the underlying messenger contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function cancelOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        address bridgeAdapter_,
        bytes calldata bridgeAdapterArgs_
    ) external payable;

    /**
     * @notice Cancel an order on behalf of the recipient
     * @dev Can be called by anyone with a valid signature from the order's recipient
     * @param orderId_ ID of the order to cancel
     * @param orderData_ OrderData payload with all order information required to identify an order to be cancelled
     * @param signature_ Order sender's signature of the EIP-712 payload (see getCancelOrderDigest)
     * @dev   The payable amount is forwarded to the underlying messenger contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function cancelOrderFor(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata signature_
    ) external payable;

    /**
     * @notice Cancel an order on behalf of the recipient with additional message data required by some crosschain messages
     * @dev Can be called by anyone with a valid signature from the order's recipient
     * @param orderId_ ID of the order to cancel
     * @param orderData_ OrderData payload with all order information required to identify an order to be cancelled
     * @param signature_ Order sender's signature of the EIP-712 payload (see getCancelOrderDigest)
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @dev   The payable amount is forwarded to the underlying messenger contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function cancelOrderFor(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata signature_,
        bytes calldata bridgeAdapterArgs_
    ) external payable;

    /**
     * @notice Cancel an order on behalf of the recipient with additional message data required by some crosschain messages
     * @dev Can be called by anyone with a valid signature from the order's recipient
     * @param orderId_ ID of the order to cancel
     * @param orderData_ OrderData payload with all order information required to identify an order to be cancelled
     * @param signature_ Order sender's signature of the EIP-712 payload (see getCancelOrderDigest)
     * @param bridgeAdapter_ Address of the bridge adapter to use for crosschain messages (must be supported by Portal V2)
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @dev   The payable amount is forwarded to the underlying messenger contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function cancelOrderFor(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata signature_,
        address bridgeAdapter_,
        bytes calldata bridgeAdapterArgs_
    ) external payable;

    /* ========== Filling Orders ========== */

    /**
     * @notice Fill an order on this chain
     * @param orderId_ ID of the order to fill
     * @param orderData_ OrderData payload with all order information required to identify an order to be filled
     * @param fillerParams_ Parameters supplied by the solver of the order
     * @dev   The orderData is packed and hashed to verify the order ID as a safeguard for solvers
     * @dev   The payable amount is forwarded to the underlying messenger contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_
    ) external payable;

    /**
     * @notice Fill an order on this chain with additional message data required by some crosschain messages
     * @param orderId_ ID of the order to fill
     * @param orderData_ OrderData payload with all order information required to identify an order to be filled
     * @param fillerParams_ Parameters supplied by the solver of the order
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @dev   The orderData is packed and hashed to verify the order ID as a safeguard for solvers
     * @dev   The payable amount is forwarded to the underlying messenger contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_,
        bytes calldata bridgeAdapterArgs_
    ) external payable;

    /**
     * @notice Fill an order on this chain with additional message data required by some crosschain messages
     * @param orderId_ ID of the order to fill
     * @param orderData_ OrderData payload with all order information required to identify an order to be filled
     * @param fillerParams_ Parameters supplied by the solver of the order
     * @param bridgeAdapter_ Address of the bridge adapter to use for crosschain messages (must be supported by Portal V2)
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @dev   The orderData is packed and hashed to verify the order ID as a safeguard for solvers
     * @dev   The payable amount is forwarded to the underlying messenger contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_,
        address bridgeAdapter_,
        bytes calldata bridgeAdapterArgs_
    ) external payable;

    /**
     * @notice Report a fill that was made on another chain back to this chain as the origin chain
     * @dev Must be called by the messenger contract
     * @param report_ Fill data sent from the destination chain
     */
    function reportFill(FillReport calldata report_) external;

    /**
     * @notice Report a cross-chain cancellation of an order.
     * @dev Must be called by the messenger contract
     * @param report_ Cancel data sent from the destination chain
     */
    function reportCancel(CancelReport calldata report_) external;

    /* ========== Admin Functions ========== */

    /**
     * @notice Set external chain support for orders
     * @dev Must be DEFAULT_ADMIN_ROLE to call
     * @param destChainId_ The chain ID for the destination chain used by the messenger
     * @param isSupported_ whether support for the chain should be enabled (true activates, false deactivates)
     */
    function setDestinationSupported(uint32 destChainId_, bool isSupported_) external;

    /* ========== View Functions ========== */

    /**
     * @notice Returns the order ID for the provided OrderData payload
     * @dev The order ID is a unique value across all supported chains
     */
    function getOrderId(OrderData calldata orderData_) external pure returns (bytes32);

    /**
     * @notice Returns the state of a local order (i.e. one that originated on this chain)
     * @dev The order must have originated on this chain or the information will not be available
     */
    function getOrder(bytes32 orderId_) external view returns (Order memory);

    /**
     * @notice Returns the amount out filled and amount in released for an order with this chain as the destination
     * @dev The order must be settled on this chain or the information will not be available
     */
    function getFilledAmounts(bytes32 orderId_) external view returns (FilledAmounts memory);

    /// @notice Returns the next nonce for the provided sender address
    function getSenderNonce(address sender_) external view returns (uint64);

    /// @notice Returns whether orders can be created with the provided chain ID as the destination
    function isDestinationSupported(uint32 destChainId_) external view returns (bool);

    /* ========== EIP-712 Digest Functions ========== */

    /**
     * @notice Returns the EIP-712 digest that a user must sign to open a gasless order
     * @param params_ gasless order creation parameters (see GaslessOrderParams definition)
     */
    function getGaslessOrderDigest(GaslessOrderParams memory params_) external view returns (bytes32);

    /**
     * @notice Returns the EIP-712 digest that a user must sign to cancel orders gaslessly
     * @param orderId_ ID of the order to cancel
     */
    function getCancelOrderDigest(bytes32 orderId_) external view returns (bytes32);
}
