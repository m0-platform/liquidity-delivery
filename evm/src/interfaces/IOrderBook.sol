// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8;

interface IOrderBook {
    // TODO NatSpec for all events and methods

    // ========== Events ========== //
    event OrderOpen(bytes32 orderId, address tokenIn, uint128 amountIn, uint32 indexed destChainId, bytes32 indexed tokenOut, uint128 amountOut, bytes32 indexed solver);
    event Fill(bytes32 orderId, address indexed solver, uint128 amountOutFilled);
    event CancelRequest(bytes32 orderId, uint40 newFillDeadline);
    event RefundClaimed(bytes32 orderId, address indexed sender, uint128 amountInRefunded);
    event OrderCompleted(bytes32 orderId);

    // ========== Errors ========== //
    error AmountInZero();
    error AmountOutZero();
    error InvalidDeadline();
    error InvalidDestinationChain();
    error InvalidFinalityBuffer();
    error InvalidOrderStatus();
    error InvalidOrderVersion();
    error NotAuthorized();
    error OrderExpired();
    error OrderFilled();
    error OrderIdMismatch();
    error RefundPending();

    // ========== Structs and Enums ========== //
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

    // struct GaslessOrderParams {
    //     address originContract; // the contract address on the origin chain that this order can be submitted to
    //     uint32 originChainId;
    //     address originToken;
    //     uint32 destinationChainId;
    //     bytes32 destinationToken;  // 32 bytes used for addresses to accomodate SVM chains
    //     address sender;
    //     bytes32 recipient;
    //     uint40 openDeadline; // order must be opened by this time
    //     uint40 fillDeadline; // order can be filled up to this time, remaining funds can be refunded after this
    //     uint256 amount; // amountIn == amountOut since all conversions are 1:1        
    // }

    enum OrderStatus {
        DoesNotExist,
        Created,
        CancelRequested,
        Completed
    }

    // Complete data about an order
    struct Order {
        OrderStatus status;
        uint16 version; // version of the contract
        uint32 destChainId;
        uint40 fillDeadline; // timestamp by which the order must be filled on the destination chain
        uint64 nonce; // a counter tied to the sender to allow unique
        address tokenIn;
        bytes32 tokenOut;
        address sender; // address that provided the funds on the origin chain
        bytes32 recipient; // address to receive the funds on the destination chain
        uint128 amountIn;
        uint128 amountOut;
        bytes32 solver; // may be the zero address, in which case the order can be filled by any approved solver
    }

    // Data needed to compute a unique ID and fill an order on a destination chain
    // It can be populated from the complete Order data
    // Addresses are all 32 bytes to support both EVM and SVM environments
    // Order ID is computed as the keccak256 hash of the abi-encoding of this struct
    // bytes32 orderId = keccak256(abi.encode(orderData));
    struct OrderData {
        uint16 version; // version of the contract, prevents replays of prior orders on new deployments		
        uint32 originChainId;
        bytes32 sender;
        uint64 nonce; // we could stop here and have a unique ID, however, including the delivery information allows us to check it on the destination
        uint32 destChainId;
        uint64 fillDeadline; 
        uint128 amountOut;
        bytes32 tokenOut;
        bytes32 recipient;
        bytes32 solver; // may be the zero address, in which case the order can be filled by any approved solver	
    }

    struct FillReport {
        bytes32 orderId;
        uint128 amountOutFilled; // amount the solver filled on the destination chain
        bytes32 originRecipient; // address on the origin chain that should receive the released funds
    }

    struct FillParams {
        uint128 amountOutToFill;
        bytes32 originRecipient;
    }

    struct Destination {
        bool isSupported;
        uint40 finalityBuffer; // number of seconds to wait after the fill deadline before allowing refunds
    }

    /// @notice Opens an order
	/// @dev To be called by the user
	/// @dev This method must emit the Open event
	/// @param orderParams The OnchainOrderParams definition
    /// @return The unique ID of the opened order
	function openOrder(OnchainOrderParams calldata orderParams) external returns (bytes32);

    // /// @notice Opens a gasless order on behalf of a user.
	// /// @dev To be called by the filler.
	// /// @dev This method must emit the Open event
	// /// @param orderParams The GaslessOrderParams definition
	// /// @param signature The user's signature over the order
	// function openFor(GaslessOrderParams calldata orderParams, bytes calldata signature) external;

    /// @notice Request cancellation of an order before its fill deadline
    /// @dev To be called by the order sender
    function requestCancelOrder(bytes32 orderId) external;

    /// @notice Refund any remaining unfilled amount of an order after its fill deadline + finality buffer has passed
    function claimRefund(bytes32 orderId) external;

    /// @notice Fill an order on this chain
    function fillOrder(bytes32 orderId, OrderData calldata orderData, FillParams calldata fillerParams) external;

    /// @notice Report a fill that was made on another chain back to this chain as the origin chain
    /// @dev To be called by a permitted messenger contract
    function reportFill(FillReport calldata report) external;

    // ========== Admin Functions ========== //

    function setDestinationConfig(uint32 destChainId, bool isSupported, uint40 finalityBuffer) external;


    // ========== View Functions ========== //

    function getOrderId(OrderData calldata orderData) external pure returns (bytes32);

    function getOrder(bytes32 orderId) external view returns (Order memory);

    function isDestinationSupported(uint32 destChainId) external view returns (bool);

    function getDestinationFinalityBuffer(uint32 destChainId) external view returns (uint40);

}