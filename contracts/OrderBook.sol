// SPDX-License-Identifier: GPL-3.0

pragma solidity 0.8.26;

interface IOrderBook {
    event OrderOpen(bytes32 orderId, address tokenIn, uint256 amountIn, uint32 indexed destChainId, bytes32 indexed tokenOut, uint256 amountOut, bytes32 indexed solver);
    event Fill(bytes32 orderId, bytes32 indexed solver, uint256 amountOutFilled);
    event CancelRequest(bytes32 orderId, uint40 newFillDeadline);
    event RefundClaimed(bytes32 orderId, address indexed sender, uint256 amountInRefunded);
    event OrderCompleted(bytes32 orderId);

    struct OnchainOrderParams {
        address tokenIn; 
        uint32 destChainId;
        bytes32 tokenOut; // 32 bytes used for addresses to accomodate SVM chains in the network
        uint256 amountIn;
        uint256 amountOut;
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
        uint32 destChainId;
        uint40 fillDeadline; // timestamp by which the order must be filled on the destination chain
        uint64 nonce; // a counter tied to the sender to allow unique
        address tokenIn;
        bytes32 tokenOut;
        address sender; // address that provided the funds on the origin chain
        bytes32 recipient; // address to receive the funds on the destination chain
        uint256 amountIn;
        uint256 amountOut;
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
        uint40 fillDeadline; 
        bytes32 tokenOut;
        bytes32 recipient;
        uint256 amountOut;
        bytes32 solver; // may be the zero address, in which case the order can be filled by any approved solver	
    }

    struct FillReport {
        bytes32 orderId;
        uint256 amountOutFilled; // amount the solver filled on the destination chain
        bytes32 originRecipient; // address on the origin chain that should receive the released funds
    }

    struct FillerParams {
        uint256 amountOutToFill;
        bytes32 originRecipient;
    }

    /// @notice Opens an order
	/// @dev To be called by the user
	/// @dev This method must emit the Open event
	/// @param orderParams The OnchainOrderParams definition
	function open(OnchainOrderParams calldata orderParams) external;

    // /// @notice Opens a gasless order on behalf of a user.
	// /// @dev To be called by the filler.
	// /// @dev This method must emit the Open event
	// /// @param orderParams The GaslessOrderParams definition
	// /// @param signature The user's signature over the order
	// function openFor(GaslessOrderParams calldata orderParams, bytes calldata signature) external;

    /// @notice Request cancellation of an order before its fill deadline
    /// @dev To be called by the order sender
    function requestCancel(bytes32 orderId) external;

    /// @notice Refund any remaining unfilled amount of an order after its fill deadline + finality buffer has passed
    function claimRefund(bytes32 orderId) external;

    /// @notice Fill an order on this chain
	function fill(OrderData calldata orderData) external;

    /// @notice Report a fill that was made on another chain back to this chain as the origin chain
    /// @dev To be called by a permitted messenger contract
    function reportFill(FillReport calldata report) external;
}

interface IMessenger {
    function sendFillReport(uint32 destinationChainId, IOrderBook.FillReport calldata report) external;
}


contract OrderBook is IOrderBook {

    // ========== Errors ========== //
    // TODO use custom errors vs. strings for gas savings

    // ========== State Variables ========== //

    /// @notice the chain ID of this chain according to the messaging network used by this contract
    uint32 public chainId; 

    // version of the limit order system
    uint16 public constant version = 1;

    // TODO this messaging setup is unclear, but this is a simple stand-in
    // sends crosschain messages to report fills on this chain to other chains
    // receive crosschain messages to report fills on other chains to this chain
    address public messenger;
    // Alternative: chain-specific messengers
    // mapping(address => uint32) public messengerOriginId;// messenger contract address => this chain's origin ID for that messenger (it can be different for different messaging networks)
    // mapping(uint32 => address) public chainMessenger; // chain ID => contract to use for sending messages to and receiving messages from that chain

    // chain ID => number of seconds to wait for finality after fill deadline before allowing refunds
    mapping(uint32 destChainId => uint40 finalityBuffer) public destChainFinalityBuffer; 

    // only store full data about origin orders
    mapping(bytes32 orderId => Order) public localOrders;

    // store fill amounts for both origin and destination orders
    mapping(bytes32 orderId => uint256 fillAmount) public orderAmountOutFilled;

    // track nonces for each sender to ensure unique order IDs
    mapping(address sender => uint64 nonce) public senderNonces;

    // // TODO do we want to restrict tokens can be used?
    // mapping(address => bool) public acceptOrdersIn; // whether to accept orders with a given origin token
    // mapping(address => bool) public acceptFillsIn; // whether to accept fills with a given destination token

    // ========== Constructor ========== //

    constructor() {
        chainId = uint32(block.chainid); // TODO replace with messaging network chain ID if different
    }


    // ========== Initiating Orders ========== //

    function open(OnchainOrderParams calldata orderParams) external override returns (bytes32 orderId) {
        // Validate order parameters

        if (orderParams.fillDeadline < block.timestamp) {
            revert("OrderBook: fill deadline invalid");
        }

        // TODO should we store a list of valid destination chains? What about destination tokens?
        // With the fillDeadline boundaries, the worst case is that an unsupported order can't be filled and is refunded after expiry
        // However, that may not be great UX

        // Create order
        uint256 nonce = senderNonces[msg.sender]++;

        orderId = getOrderId(OrderData({
            version: version, // origin contract version
            originChainId: chainId,
            sender: msg.sender,
            nonce: nonce,
            destChainId: orderParams.destChainId,
            fillDeadline: orderParams.fillDeadline,
            tokenOut: orderParams.tokenOut,
            recipient: orderParams.recipient,
            amountOut: orderParams.amountOut,
            solver: orderParams.solver
        }));

        localOrders[orderId] = Order({
            version: version, // origin contract version
            status: OrderStatus.Created,
            destChainId: orderParams.destChainId,
            fillDeadline: orderParams.fillDeadline,
            nonce: nonce,
            tokenIn: orderParams.tokenIn,
            tokenOut: orderParams.tokenOut,
            sender: msg.sender,
            recipient: orderParams.recipient,
            amountIn: orderParams.amountIn,
            amountOut: orderParams.amountOut,
            solver: orderParams.solver
        });

        // Transfer tokens in from the sender
        IERC20(orderParams.tokenIn).transferFrom(msg.sender, address(this), orderParams.amountIn);

        emit OrderOpen(orderId, orderParams.tokenIn, orderParams.amountIn, orderParams.destChainId, orderParams.tokenOut, orderParams.amountOut, orderParams.solver);
    }

    // ========== Refunding Orders ========== //

    function requestCancel(bytes32 orderId) external override {
        Order storage order = localOrders[orderId];

        // Validate that the order can be cancelled
        if (order.status != OrderStatus.Created) {
            revert("OrderBook: order not active");
        }

        if (order.sender != msg.sender) {
            revert("OrderBook: caller is not order sender");
        }

        if (order.fillDeadline < block.timestamp) {
            revert("OrderBook: order fill deadline passed. refund already available");
        }

        // Mark the order as cancel requested
        order.status = OrderStatus.CancelRequested;

        // Set the fill deadline to the current time
        // This will allow the caller to claim a refund after the finality buffer has passed
        order.fillDeadline = uint40(block.timestamp);
        
        emit CancelRequest(orderId, order.fillDeadline);
    }


    // Note: this function allows anyone to trigger a refund of an order after its fill deadline + finality buffer has passed
    // This allows applications to gracefully handle refunds for orders that weren't filled 
    // Alternatively, if a user requested a refund, they can claim it here
    function claimRefund(bytes32 orderId) external override {
        Order storage order = localOrders[orderId];

        // Validate that the order can be refunded
        if (order.status != OrderStatus.Created || order.status != OrderStatus.CancelRequested) {
            revert("OrderBook: order not active or cancel requested");
        }

        // Check that the fill deadline + finality buffer has passed
        uint40 finalityBuffer = destChainFinalityBuffer[order.destChainId];
        if (order.fillDeadline + finalityBuffer >= block.timestamp) {
            revert("OrderBook: order fill deadline not yet passed");
        }


        // Calculate the refund amount
        uint256 outFilled = orderAmountOutFilled[orderId];
        uint256 outRemaining = order.amountOut - outFilled;

        if (outRemaining == 0) {
            revert("OrderBook: order already fully filled");
        }

        // TODO need to think about rounding and precision loss with different token decimal values
        uint256 inRemaining = outFilled == 0 ? order.amountIn : (order.amountIn * outRemaining) / order.amountOut;


        // Update the order amountIn and amountOut values to reflect the refund
        // This prevents double refunds if this function is called again
        order.amountIn -= inRemaining;
        order.amountOut -= outRemaining;

        // Set the order status to completed
        order.status = OrderStatus.Completed;

        // Transfer the remaining amount back to the sender
        IERC20(order.originToken).transfer(order.sender, inRemaining);

        emit RefundClaimed(orderId, order.sender, inRemaining);
    }


    // ========== Filling Orders ========== //
    function fill(bytes32 orderId, OrderData calldata orderData, FillerParams calldata fillerParams) external override {
        // Validate fill data
        if (chainId != orderData.destChainId) {
            revert("OrderBook: fill data destination chain ID does not match protocol chain ID");
        }

        if (orderData.fillDeadline < block.timestamp) {
            revert("OrderBook: order fill deadline passed");
        }

        // If the solver is specified, ensure that the caller is the designated solver
        address solver = address(uint160(uint256(orderData.solver)));
        if (solver != address(0) && solver != msg.sender) {
            revert("OrderBook: caller is not designated solver for order");
        }

        // Ensure the provided order ID matches the computed order ID from the order data
        // This check is not strictly required, but it is a useful sanity check for solvers
        // to ensure they have the order data correct
        bytes32 orderId_ = getOrderId(orderData);

        if (orderId != orderId_) {
            revert("OrderBook: order ID does not match fill data");
        }

        // Calculate fill amount as the minimum of the filler provided amount and the remaining unfilled amount
        uint256 outFilled = orderAmountOutFilled[orderId];
        uint256 outRemaining = orderData.amountOut - outFilled;
        if (outRemaining == 0) {
            revert("OrderBook: order already fully filled");
        }
        bool fullFill = fillerParams.amountOutToFill >= outRemaining;
        uint256 fillAmount = fullFill ? outRemaining : fillerParams.amountOutToFill;

        // Update order fill amount
        orderAmountOutFilled[orderId] += fillAmount;

        // Handle releasing the corresponding amount of origin tokens to the filler
        if (chainId == orderData.originChainId) {
            // If a full fill, mark the order as completed
            if (fullFill) {
                localOrders[orderId].status = OrderStatus.Completed;
                emit OrderCompleted(orderId);
            }

            // Calculate the amount of origin tokens to release to the filler
            Order storage order = localOrders[orderId];
            // TODO same concerns about rounding and precision loss with different token decimal values
            uint256 inToRelease = order.amountOut == fillAmount ? order.amountIn : (order.amountIn * fillAmount) / order.amountOut;

            // If this is a fill on the origin chain, we can immediately release the corresponding amount of origin tokens to the recipient
            // This is because the origin and destination chains are the same, so no cross-chain messaging is needed
            IERC20(order.tokenIn).transferFrom(address(this), address(uint160(uint256(fillerParams.originRecipient))), inToRelease);
        }
        
        // Transfer tokens from the solver to the recipient
        IERC20(orderData.tokenOut).transferFrom(msg.sender, address(uint160(uint256(orderData.recipient))), fillAmount);
        
        // This block is split out to allow the above transfer to happen before any cross-chain messaging
        if (chainId != orderData.originChainId) {
            // If this is a fill on a different chain than the origin chain, we need to send a message back to the origin chain to release the corresponding amount of origin tokens to the recipient
            // TODO implement cross-chain messaging to report the fill back to the origin chain
            // TODO determine best method for batching these messages
            IMessenger(messenger).sendFillReport(
                orderData.originChainId,
                FillReport({
                    orderId: orderId,
                    originRecipient: fillerParams.originRecipient,
                    amountOutFilled: fillAmount
                })
            );
        }

        emit Fill(orderId, msg.sender, fillAmount);
    }

    // ========== Receiving Fill Reports ========== //

    // TODO allow batch reporting to save gas and reduce the number of required messages
    function reportFill(FillReport calldata report) external override {
        Order storage order = localOrders[report.orderId];

        if (order.originChainId != chainId) {
            revert("OrderBook: order origin chain ID does not match protocol chain ID");
        }

        if (msg.sender != messenger) {
            revert("OrderBook: caller is not permitted messenger for destination chain");
        }

        // Update the fill amount for the order
        orderAmountOutFilled[report.orderId] += report.amountOutFilled;
        uint256 outFilled = orderAmountOutFilled[report.orderId];
        if (outFilled == order.amountOut) {
            order.status = OrderStatus.Completed;
            emit OrderCompleted(report.orderId);
        }

        // Calculate the corresponding amount of origin tokens to release to the solver's designated recipient
        // TODO same concerns about rounding and precision loss with different token decimal values
        uint256 inToRelease = order.amountOut == report.amountOutFilled ? order.amountIn : (order.amountIn * report.amountOutFilled) / order.amountOut;

        // Transfer the corresponding amount of origin tokens to the filler
        IERC20(order.tokenIn).transferFrom(address(this), address(uint160(uint256(report.originRecipient))), inToRelease);
    }


    // Order IDs are unique across chains and allow using fill data to compute the identifier
    // This is useful for tracking data against orders on both the origin and destination chains
    function getOrderId(OrderData calldata orderData) internal pure returns (bytes32) {
        return keccak256(abi.encode(orderData));
    }
}
