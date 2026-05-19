// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.0;

interface IOrderBook {
    /* ========== Events ========== */

    /**
     * @notice Emitted when a new order is opened
     * @dev This event is emitted on the origin chain
     * @param orderId The ID of the order
     * @param funder The address that provided the input funds for the order
     * @param sender The address that owns the order on the origin (this) chain
     * @param tokenIn The address of the input token on the origin (this) chain
     * @param amountIn The amount of input token provided
     * @param destChainId The internal chain ID where the order will be filled
     * @param tokenOut The address of the output token on the destination chain
     * @param amountOut The amount of output token expected
     * @param solver The address of the solver that will fill the order, or zero address if any solver can fill
     * @param fillDeadline Timestamp by which the order must be filled on the destination chain
     */
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

    /**
     * @notice Emitted when an order is filled
     * @dev This event is emitted on the destination chain
     * @param orderId The ID of the order being filled
     * @param solver The address of the solver that filled the order
     * @param amountOutFilled The amount of output token that was filled
     * @param amountInToRelease The amount of input token they will receive on the origin chain
     * @param messageId The ID of the crosschain message reporting this fill back to the origin chain (zero for same-chain fills)
     */
    event OrderFilled(
        bytes32 indexed orderId,
        address indexed solver,
        uint128 amountInToRelease,
        uint128 amountOutFilled,
        bytes32 indexed messageId
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
     * @param messageId The ID of the crosschain message reporting this cancellation back to the origin chain (zero for same-chain cancels)
     */
    event OrderCancelled(bytes32 indexed orderId, bytes32 indexed messageId);

    /**
     * @notice Emitted when the support for a destination chain is updated
     * @dev This event is emitted on the origin chain
     * @param destChainId The internal chain ID of the destination chain
     * @param isSupported Whether orders can be created with this chain as the destination
     */
    event DestinationSupportUpdated(uint32 indexed destChainId, bool isSupported);

    /**
     * @notice Emitted when a fill is reported from a destination chain
     * @dev This event is emitted on the origin chain
     * @param orderId The ID of the order that was filled
     * @param originRecipient The address on the origin chain that received released funds
     * @param amountInToRelease The amount of input token released to the filler on the origin chain
     * @param amountOutFilled The amount of output token that was filled on the destination chain
     */
    event FillReported(
        bytes32 indexed orderId,
        address indexed originRecipient,
        uint128 amountInToRelease,
        uint128 amountOutFilled
    );

    /**
     * @notice Emitted when a cancellation is reported from a destination chain
     * @dev This event is emitted on the origin chain
     * @param orderId The ID of the cancelled order
     */
    event CancelReported(bytes32 indexed orderId);

    /* ========== Errors ========== */
    error AmountInZero();
    error AmountOutZero();
    error FillAmountZero();
    error InvalidDeadline();
    error InvalidDestinationChain();
    error InvalidMsgValue();
    error InvalidOrderStatus();
    error InvalidOrderVersion();
    error InvalidRecipient();
    error InvalidSolver();
    error InvalidReport();
    error InvalidTimestamp();
    error InvalidReportSource();
    error NotAuthorized();
    error OrderExpired();
    error OrderAlreadyExists();
    error OrderIdMismatch();
    error SameTokenOrder();
    error ZeroAdmin();
    error ZeroPauser();
    error ZeroPortal();
    error ZeroSender();

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
     * @param sender Address that will own the order (for cancellation rights and refunds). Tokens are pulled from msg.sender.
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
        address sender;
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
     * @dev This struct is sent by the portal contract to report fills that occurred
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
     * @dev This struct is sent by the portal contract to report order cancellations and refunds
     *      that occurred on the destination chain back to the origin chain for processing
     * @param orderId The ID of the order that a cancellation is being reported for
     * @param orderSender The address on the origin chain that created the order
     * @param tokenIn The address of the input token on the origin chain
     * @param amountInToRefund The amount of input token to refund to the order sender on the origin chain
     * The last two are included for non-EVM chains to provide a way to resolve the sender and token
     */
    struct CancelReport {
        bytes32 orderId;
        bytes32 orderSender;
        bytes32 tokenIn;
        uint128 amountInToRefund;
    }

    /**
     * @notice Parameters supplied by the filler of an order
     * @dev This struct contains parameters that are specific to the filler
     *      and are not part of the original order data
     * @param amountOutToFill The amount of output token the filler is providing to fill
     * @param originRecipient The address on the origin chain that should receive released funds
     * @param refundAddress (optional) The address to send any bridge refund costs to.
     *                       See PortalV2 for more info. If not provided (i.e. zero address),
     *                       it defaults to msg.sender. Not required for same chain fills.
     */
    struct FillParams {
        uint128 amountOutToFill;
        bytes32 originRecipient;
        bytes32 refundAddress;
    }

    /**
     * @notice Data structure to track filled amounts for an order on the origin and destination chains
     * @param amountInRefunded Amount of input token refunded to the order sender
     * @param amountInReleased Amount of input token released to solver(s)
     * @param amountOutFilled Amount of output token filled by solver(s)
     */
    struct FilledAmounts {
        uint128 amountInRefunded;
        uint128 amountInReleased;
        uint128 amountOutFilled;
    }

    /* ========== Creating Orders ========== */

    /**
     * @notice Opens an order
     * @dev Must be called by the user providing the input funds
     * @param orderParams_ order creation parameters (see OrderParams definition)
     * @return orderId_ The unique ID of the opened order
     */
    function openOrder(OrderParams calldata orderParams_) external returns (bytes32 orderId_);

    /**
     * @notice Opens an order with an EIP-2612 permit signature for token approval
     * @dev Must be called by the user providing the input funds
     * @param orderParams_ order creation parameters (see OrderParams definition)
     * @param deadline_ deadline for the permit signature
     * @param v_ v parameter of the permit signature
     * @param r_ r parameter of the permit signature
     * @param s_ s parameter of the permit signature
     * @return orderId_ The unique ID of the opened order
     */
    function openOrderWithPermit(
        OrderParams calldata orderParams_,
        uint256 deadline_,
        uint8 v_,
        bytes32 r_,
        bytes32 s_
    ) external returns (bytes32 orderId_);

    /**
     * @notice Opens an order with an EIP-2612 permit signature for token approval
     * @dev Must be called by the user providing the input funds
     * @param orderParams_ order creation parameters (see OrderParams definition)
     * @param deadline_ deadline for the permit signature
     * @param permitSignature_ packed encoding of the permit signature
     * @return orderId_ The unique ID of the opened order
     */
    function openOrderWithPermit(
        OrderParams calldata orderParams_,
        uint256 deadline_,
        bytes memory permitSignature_
    ) external returns (bytes32 orderId_);

    /* ========== Refunding Orders ========== */

    /**
     * @notice Cancel an order
     * @dev Must be called by the order's recipient (or sender if same chain order) before fill deadline
     * @dev Can be called by anyone after the fill deadline (permissionless refunds)
     * @param orderId_ - ID of the order to cancel
     * @param orderData_ OrderData payload with all order information required to identify an order to be cancelled
     * @return messageId_ The ID of the crosschain message reporting this cancellation back to the origin chain (zero for same-chain cancels)
     * @dev   The payable amount is forwarded to the underlying portal contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function cancelOrder(bytes32 orderId_, OrderData calldata orderData_) external payable returns (bytes32 messageId_);

    /**
     * @notice Cancel an order with additional message data required by some crosschain messages
     * @dev Must be called by the order's recipient (or sender if same chain order) before fill deadline
     * @dev Can be called by anyone after the fill deadline (permissionless refunds)
     * @param orderId_ ID of the order to cancel
     * @param orderData_ OrderData payload with all order information required to identify an order to be cancelled
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @return messageId_ The ID of the crosschain message reporting this cancellation back to the origin chain (zero for same-chain cancels)
     * @dev   The payable amount is forwarded to the underlying portal contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function cancelOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata bridgeAdapterArgs_
    ) external payable returns (bytes32 messageId_);

    /**
     * @notice Cancel an order with additional message data required by some crosschain messages
     * @dev Must be called by the order's recipient (or sender if same chain order) before fill deadline
     * @dev Can be called by anyone after the fill deadline (permissionless refunds)
     * @param orderId_ ID of the order to cancel
     * @param orderData_ OrderData payload with all order information required to identify an order to be cancelled
     * @param bridgeAdapter_ Address of the bridge adapter to use for crosschain messages (must be supported by Portal V2)
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @return messageId_ The ID of the crosschain message reporting this cancellation back to the origin chain (zero for same-chain cancels)
     * @dev   The payable amount is forwarded to the underlying portal contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function cancelOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        address bridgeAdapter_,
        bytes calldata bridgeAdapterArgs_
    ) external payable returns (bytes32 messageId_);

    /* ========== Filling Orders ========== */

    /**
     * @notice Fill an order on this chain
     * @param orderId_ ID of the order to fill
     * @param orderData_ OrderData payload with all order information required to identify an order to be filled
     * @param fillerParams_ Parameters supplied by the solver of the order
     * @return messageId_ The ID of the crosschain message reporting this fill back to the origin chain (zero for same-chain fills)
     * @dev   The orderData is packed and hashed to verify the order ID as a safeguard for solvers
     * @dev   The payable amount is forwarded to the underlying portal contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_
    ) external payable returns (bytes32 messageId_);

    /**
     * @notice Fill an order on this chain with additional message data required by some crosschain messages
     * @param orderId_ ID of the order to fill
     * @param orderData_ OrderData payload with all order information required to identify an order to be filled
     * @param fillerParams_ Parameters supplied by the solver of the order
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @return messageId_ The ID of the crosschain message reporting this fill back to the origin chain (zero for same-chain fills)
     * @dev   The orderData is packed and hashed to verify the order ID as a safeguard for solvers
     * @dev   The payable amount is forwarded to the underlying portal contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_,
        bytes calldata bridgeAdapterArgs_
    ) external payable returns (bytes32 messageId_);

    /**
     * @notice Fill an order on this chain with additional message data required by some crosschain messages
     * @param orderId_ ID of the order to fill
     * @param orderData_ OrderData payload with all order information required to identify an order to be filled
     * @param fillerParams_ Parameters supplied by the solver of the order
     * @param bridgeAdapter_ Address of the bridge adapter to use for crosschain messages (must be supported by Portal V2)
     * @param bridgeAdapterArgs_ Additional data required by some crosschain message protocols (see PortalV2 for more info)
     * @return messageId_ The ID of the crosschain message reporting this fill back to the origin chain (zero for same-chain fills)
     * @dev   The orderData is packed and hashed to verify the order ID as a safeguard for solvers
     * @dev   The payable amount is forwarded to the underlying portal contract to send crosschain messages.
     *        This should be 0 for same chain fills. For crosschain fills, see the Portal V2 contract for guidance on
     *        getting a quote for the required fee
     */
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_,
        address bridgeAdapter_,
        bytes calldata bridgeAdapterArgs_
    ) external payable returns (bytes32 messageId_);

    /**
     * @notice Report a fill that was made on another chain back to this chain as the origin chain
     * @dev Must be called by the portal contract
     * @param sourceChainId_ The chain ID that the fill report was sent from
     * @param report_ Fill data sent from the destination chain
     */
    function reportFill(uint32 sourceChainId_, FillReport calldata report_) external;

    /**
     * @notice Report a cross-chain cancellation of an order.
     * @dev Must be called by the portal contract
     * @param sourceChainId_ The chain ID that the cancel report was sent from
     * @param report_ Cancel data sent from the destination chain
     */
    function reportCancel(uint32 sourceChainId_, CancelReport calldata report_) external;

    /* ========== Admin Functions ========== */

    /**
     * @notice Set external chain support for orders
     * @dev Must be DEFAULT_ADMIN_ROLE to call
     * @param destChainId_ The chain ID for the destination chain used by the portal
     * @param isSupported_ whether support for the chain should be enabled (true activates, false deactivates)
     */
    function setDestinationSupported(uint32 destChainId_, bool isSupported_) external;

    /**
     * @notice Pauses the contract.
     * @dev    Can only be called by an account with the PAUSER_ROLE.
     * @dev    When paused, all external order actions are disabled (open, fill, and cancel).
     *         However, processing of crosschain fill and cancel reports is still allowed.
     *         This enables safe upgrades of the contract by pausing all instances and
     *         waiting until inflight messages are processed.
     */
    function pause() external;

    /**
     * @notice Unpauses the contract.
     * @dev    Can only be called by an account with the PAUSER_ROLE.
     */
    function unpause() external;

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
     * @notice Returns the OrderData for a local order (i.e. one that originated on this chain)
     * @dev The order must have originated on this chain or the information will not be available
     *      This is a convenience function for solvers to get the OrderData needed to fill an order
     * @param orderId_ The ID of the order to get data for
     * @return orderData_ The OrderData struct containing all order information needed to fill
     */
    function getOrderData(bytes32 orderId_) external view returns (OrderData memory orderData_);

    /**
     * @notice Returns the amount out filled and amount in released for an order with this chain as the destination
     * @dev The order must be settled on this chain or the information will not be available
     */
    function getFilledAmounts(bytes32 orderId_) external view returns (FilledAmounts memory);

    /// @notice Returns the next nonce for the provided sender address
    function getSenderNonce(address sender_) external view returns (uint64);

    /// @notice Returns whether orders can be created with the provided chain ID as the destination
    function isDestinationSupported(uint32 destChainId_) external view returns (bool);
}
