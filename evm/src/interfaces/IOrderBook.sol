// SPDX-License-Identifier: GPL-3.0
pragma solidity >=0.8;

interface IOrderBook {
    

    /* ========== Events ========== */

    /**
     * @notice Emitted when a new order is opened
     * @dev This event is emitted on the origin chain
     * @param orderId The ID of the order
     * @param tokenIn The address of the input token on this chain
     * @param amountIn The amount of input token provided
     * @param destChainId The internal chain ID where the order will be filled
     * @param tokenOut The address of the output token on the destination chain
     * @param amountOut The amount of output token expected
     * @param solver The address of the solver that will fill the order, or zero address if any approved solver can fill
     */
    event OrderOpen(
        bytes32 orderId,
        address tokenIn,
        uint128 amountIn,
        uint32 indexed destChainId,
        bytes32 indexed tokenOut,
        uint128 amountOut,
        bytes32 indexed solver
    );

    /**
     * @notice Emitted when an order is filled
     * @dev This event is emitted on the destination chain
     * @param orderId The ID of the order being filled
     * @param solver The address of the solver that filled the order
     * @param amountOutFilled The amount of output token that was filled
     */
    event Fill(
        bytes32 indexed orderId, 
        address indexed solver, 
        uint128 amountOutFilled
    );

    /**
     * @notice Emitted when a cancellation is requested for an order by the sender
     * @dev This event is emitted on the origin chain
     * @param orderId The ID of the order being cancelled
     * @param cancelRequestedAt The timestamp when the cancellation was requested
     */
    event CancelRequested(bytes32 orderId, uint40 cancelRequestedAt);

    /**
     * @notice Emitted when a refund is claimed for an order
     * @dev This event is emitted on the origin chain
     * @param orderId The ID of the order being refunded
     * @param sender The address that the refund is sent to
     * @param amountInRefunded The amount of input token that was refunded
     */
    event RefundClaimed(
        bytes32 orderId, 
        address indexed sender, 
        uint128 amountInRefunded
    );

    /**
     * @notice Emitted when an order is completed (fully filled)
     * @dev This event is emitted on the destination chain
     * @param orderId The ID of the completed order
     */
    event OrderCompleted(bytes32 orderId);

    /* ========== Errors ========== */
    error AmountInZero();
    error AmountOutZero();
    error FinalityPending();
    error InvalidDeadline();
    error InvalidDestinationChain();
    error InvalidFinalityBuffer();
    error InvalidOrderStatus();
    error InvalidOrderVersion();
    error NotAuthorized();
    error OrderExpired();
    error OrderFilled();
    error OrderIdMismatch();

    /* ========== Structs and Enums ========== */

    /**
     * @notice Parameters required to open an order onchain
     * @dev Addresses on the destination chain are stored as bytes32 to support non-EVM
     * @param tokenIn Address of the input token on this chain
     * @param destChainId Destination chain ID where the order is to be filled
     * @param tokenOut Address of the output token on the destination chain
     * @param amountIn Amount of input token provided
     * @param amountOut Amount of output token expected
     * @param recipient Address to receive the funds on the destination chain
     * @param fillDeadline Timestamp by which the order must be filled on the destination chain
     * @param solver Address of the solver that will fill the order, or zero address if
     */
    struct OnchainOrderParams {
        address tokenIn; 
        uint32 destChainId;
        bytes32 tokenOut; // 32 bytes used for addresses to accomodate SVM chains in the network
        uint128 amountIn;
        uint128 amountOut;
        bytes32 recipient;	
        uint40 fillDeadline; // order can be filled up to this time, remaining funds can be refunded after this
        bytes32 solver; // may be the zero address, in which case the order can be filled by any approved solver
    }

    /**
     * @notice Parameters required to open a gasless order onchain
     * @dev Addresses on the destination chain are stored as bytes32 to support non-EVM
     * @dev This payload must be included in an EIP-712 payload that is then signed by the order sender
     * @param originChainId internal chain ID where the order is created (must be this chain)
     * @param tokenIn Address of the input token on the origin chain
     * @param destChainId internal chain ID where the order is to be filled
     * @param tokenOut Address of the output token on the destination chain
     * @param amountIn Amount of input token provided
     * @param amountOut Amount of output token expected
     * @param sender Address that provided the funds on the origin chain, must sign the payload
     * @param recipient Address to receive the funds on the destination chain
     * @param openDeadline Timestamp by which the order must be opened on the origin chain
     * @param fillDeadline Timestamp by which the order must be filled on the destination chain
     * @param solver Address of the solver that will fill the order, or zero address if any approved solver can fill
     */
    struct GaslessOrderParams {
        uint32 originChainId;
        address tokenIn;
        uint32 destChainId;
        bytes32 tokenOut;
        uint128 amountIn;
        uint128 amountOut;
        address sender;
        bytes32 recipient;
        uint40 openDeadline;
        uint40 fillDeadline;
        bytes32 solver;
    }

    /**
     * @notice Possible order statuses.
     */
    enum OrderStatus {
        DoesNotExist,
        Created,
        CancelRequested,
        Completed
    }

    /**
     * @notice Complete data about an order originated on this chain
     * @dev Addresses on the destination chain are stored as bytes32 to support non-EVM chains
     * @param status Current status of the order
     * @param version Version of the contract when the order was created
     * @param destChainId Destination chain ID where the order is to be filled
     * @param fillDeadline Timestamp by which the order must be filled on the destination chain
     * @param refundRequestedAt Timestamp when the refund was requested, 0 if no refund requested
     * @param nonce A counter tied to the sender to allow unique orders
     * @param tokenIn Address of the input token on this chain
     * @param tokenOut Address of the output token on the destination chain
     * @param sender Address that provided the funds on the origin chain
     * @param recipient Address to receive the funds on the destination chain
     * @param amountIn Amount of input token provided
     * @param amountOut Amount of output token expected
     * @param solver Address of the solver that will fill the order, or zero address if any approved solver can fill
     */
    struct Order {
        OrderStatus status;
        uint16 version;
        uint32 destChainId;
        uint40 fillDeadline;
        uint40 refundRequestedAt;
        uint64 nonce;
        address tokenIn;
        bytes32 tokenOut;
        address sender; 
        bytes32 recipient;
        uint128 amountIn;
        uint128 amountOut;
        bytes32 solver;
    }

    /**
     * @notice Data required to identify and fill an order on a destination chain
     * @dev This struct is used to compute a unique order ID and to provide all necessary
     *      information to fill the order on the destination chain.
     *      The order ID is computed as the keccak256 hash of the packed-encoding of this struct.
     * @param version Version of the contract when the order was created
     * @param originChainId internal chain ID where the order was created
     * @param sender Address that provided the funds on the origin chain
     * @param nonce A counter tied to the sender to allow unique orders
     * @param destChainId Destination chain ID where the order is to be filled
     * @param fillDeadline Timestamp by which the order must be filled on the destination chain
     * @param amountOut Amount of output token expected on the destination chain
     * @param tokenOut Address of the output token on the destination chain
     * @param recipient Address to receive the funds on the destination chain
     * @param solver Address of the solver that will fill the order, or zero address if any approved solver can fill
     */
    struct OrderData {
        uint16 version;
        uint32 originChainId;
        bytes32 sender;
        uint64 nonce;
        uint32 destChainId;
        uint64 fillDeadline; 
        uint128 amountOut;
        bytes32 tokenOut;
        bytes32 recipient;
        bytes32 solver;
    }

    /**
     * @notice Data reported from a destination chain back to the origin chain about a fill
     * @dev This struct is sent by the messenger contract to report fills that occurred
     *      on the destination chain back to the origin chain for refund processing.
     * @param orderId The ID of the order being reported
     * @param amountOutFilled The amount of output token that was filled on the destination chain
     * @param originRecipient The address on the origin chain that should receive released funds
     */
    struct FillReport {
        bytes32 orderId;
        uint128 amountOutFilled;
        bytes32 originRecipient;
    }

    /**
     * @notice Parameters supplied by the filler of an order
     * @dev This struct contains parameters that are specific to the filler
     *      and are not part of the original order data.
     * @param amountOutToFill The amount of output token the filler is providing to fill
     * @param originRecipient The address on the origin chain that should receive released funds
     */
    struct FillParams {
        uint128 amountOutToFill;
        bytes32 originRecipient;
    }

    /**
     * @notice Configuration for a supported destination chain
     * @param isSupported Whether orders can be created with this chain as the destination
     * @param finalityBuffer Duration (in seconds) to wait after the fill deadline before allowing refunds
     */
    struct Destination {
        bool isSupported;
        uint40 finalityBuffer;
    }

    /** 
     * @notice Opens an order
	 * @dev Must be called by the user providing the input funds
	 * @param orderParams order creation parameters (see OnchainOrderParams definition)
     * @return The unique ID of the opened order
     */
	function openOrder(OnchainOrderParams calldata orderParams) external returns (bytes32);

    /**
     * @notice Opens a gasless order on behalf of a user.
	 * @dev More flexible method relying on an offchain signature to authorize order creation
	 * @param orderParams gasless order creation parameters (see GaslessOrderParams definition)
	 * @param signature Order sender's signature of the EIP-712 payload containing the orderParams
	 */
	function openOrderFor(GaslessOrderParams calldata orderParams, bytes calldata signature) external;

    /**
     * @notice Request cancellation of an order before its fill deadline
     * @dev Must be called by the order's sender
     * @param orderId - ID of the order to cancel
     */
    function requestCancelOrder(bytes32 orderId) external;

    /**
     * @notice Request cancellation of an order before its fill deadline
     * @dev Can be called by anyone with a valid signature from the order's sender
     * @param orderId - ID of the order to cancel
     * @param signature - Order sender's signature of the EIP-712 payload containing the orderId
     */
    function requestCancelOrderFor(bytes32 orderId, bytes calldata signature) external;

    /**
     * @notice Refund any remaining unfilled amount of an order to the originator
     *         after its (fill deadline or request cancellation)
     *         timestamp  + finality buffer has passed
     * @dev    Can be called by anyone. This allows applications to gracefully
     *         handle refunds for orders that weren't filled.
     *         Alternatively, if a user requested a refund, they can claim it here.
     * @param  orderId - ID of the order to claim a refund for
     */
    function claimRefund(bytes32 orderId) external;

    /**
     * @notice Fill an order on this chain
     * @param orderId - ID of the order to fill
     * @param orderData - OrderData payload with all order information required to identify an order to be filled.
     * @param fillerParams - Parameters supplied by the solver of the order
     * @dev   The orderData is packed and hashed to verify the order ID as a safeguard for solvers.
     */
    function fillOrder(bytes32 orderId, OrderData calldata orderData, FillParams calldata fillerParams) external;

    /**
     * @notice Report a fill that was made on another chain back to this chain as the origin chain
     * @dev Must be called by the messenger contract
     * @param report - Fill data sent from the destination chain
     */
    function reportFill(FillReport calldata report) external;

    /* ========== Admin Functions ========== */

    /**
     * @notice Set external chain support and finality buffer configuration
     * @dev Must be DEFAULT_ADMIN_ROLE to call
     * @param destChainId - The chain ID for the destination chain used by the messenger
     * @param isSupported - whether support for the chain should be enabled (true activates, false deactivates)
     * @param finalityBuffer - duration (in seconds) to wait for messages from the chain to be finalized after deadlines for safe processing
     */
    function setDestinationConfig(uint32 destChainId, bool isSupported, uint40 finalityBuffer) external;

    /* ========== View Functions ========== */

    /**
     * @notice Returns the order ID for the provided OrderData payload
     * @dev The order ID is a unique value across all supported chains
     */
    function getOrderId(OrderData calldata orderData) external pure returns (bytes32);

    /**
     * @notice Returns the state of a local order (i.e. one that originated on this chain)
     * @dev The order must have originated on this chain or the information will not be available
     */
    function getOrder(bytes32 orderId) external view returns (Order memory);

    /**
     * @notice Returns the amount out that has been filled on an order with this chain as the destination
     * @dev The order must be settled on this chain (i.e. this chain is its destination) or the information will not be available
     */
    function getAmountOutFilled(bytes32 orderId) external view returns (uint128);

    /// @notice Returns whether orders can be created with the provided chain ID as the destination
    function isDestinationSupported(uint32 destChainId) external view returns (bool);

    /**
     * @notice Returns the configured finality buffer for the provided chain ID
     * @dev If a chain is not supported, this will return 0
     */
    function getDestinationFinalityBuffer(uint32 destChainId) external view returns (uint40);
}