// SPDX-License-Identifier: GPL-3.0

pragma solidity 0.8.26;

interface IOrderBook {
    struct Order { // not packed
        uint32 originChainId; // must have a consistent chain ID with the messaging network (e.g. Wormhole)
        uint32 destinationChainId;
        address originToken; 
        bytes32 destinationToken; // 32 bytes used for addresses to accomodate SVM chains in the network
        address sender;
        bytes32 recipient;	
        uint40 fillDeadline; // order can be filled up to this time, remaining funds can be refunded after this
        uint256 amount; // amountIn == amountOut since all conversions are 1:1
        uint256 amountFilled; // incremented on destination chain when fills are made and included in messages back to the source chain to allow releasing the appropriate amount of source tokens at a time
        uint256 nonce; // an incrementing value for the sender on the source chain so that the hash of the order is an unique ID
    }

    struct OnchainOrderParams {
        address originToken; 
        uint32 destinationChainId;
        bytes32 destinationToken; // 32 bytes used for addresses to accomodate SVM chains in the network
        bytes32 recipient;	
        uint40 fillDeadline; // order can be filled up to this time, remaining funds can be refunded after this
        uint256 amount; // amountIn == amountOut since all conversions are 1:1
    }

    struct GaslessOrderParams {
        address originContract; // the contract address on the origin chain that this order can be submitted to
        uint32 originChainId;
        address originToken;
        uint32 destinationChainId;
        bytes32 destinationToken;  // 32 bytes used for addresses to accomodate SVM chains
        address sender;
        bytes32 recipient;
        uint40 openDeadline; // order must be opened by this time
        uint40 fillDeadline; // order can be filled up to this time, remaining funds can be refunded after this
        uint256 amount; // amountIn == amountOut since all conversions are 1:1        
    }

    struct OrderFillData {
        uint32 originChainId;
        uint32 destinationChainId;
        address destinationToken;
        bytes32 sender;
        address recipient;
        uint32 fillDeadline;
        uint256 amount;
        uint256 nonce;
    }

    struct OrderFillReport {
        bytes32 orderId;
        address filler;
        uint256 fillAmount;
    }

    /// @notice Opens a gasless order on behalf of a user.
	/// @dev To be called by the filler.
	/// @dev This method must emit the Open event
	/// @param orderParams The GaslessOrderParams definition
	/// @param signature The user's signature over the order
	function openFor(GaslessOrderParams calldata orderParams, bytes calldata signature) external;

	/// @notice Opens an order
	/// @dev To be called by the user
	/// @dev This method must emit the Open event
	/// @param orderParams The OnchainOrderParams definition
	function open(OnchainOrderParams calldata orderParams) external;

    /// @notice Refund any remaining unfilled amount of an order after its fill deadline has passed
    function refund(bytes32 orderId) external;

    /// @notice Fill an order on this chain
	function fill(OrderFillData calldata fillData) external;

    /// @notice Report a fill that was made on another chain back to this chain as the origin chain
    /// @dev To be called by a permitted messenger contract
    function reportFill(OrderFillReport calldata report) external;
}


contract OrderBook is IOrderBook {

    // ========== Events ========== //
    event Open(bytes32 indexed orderId, uint256 amount);
    event Cancel(bytes32 indexed orderId, uint256 amountRefunded);
    event Fill(bytes32 indexed orderId, address indexed filler, uint256 amountFilled);

    // ========== Errors ========== //


    // ========== State Variables ========== //

    /// @notice The minimum duration between order open and fill deadline
    uint40 public minFillDuration;

    /// @notice The maximum duration between order open and fill deadline
    /// @dev We set a max to avoid a user setting an extremely long duration and the order being unfillable, resulting in stuck funds
    uint40 public maxFillDuration;

    /// @notice The minimum order amount
    uint256 public minOrderAmount;

    uint32 public protocolChainId; // the chain ID of this chain according to the messaging network (e.g. Wormhole) 

    mapping(uint32 => address) public chainMessenger; // chain ID => contract to use for sending messages to and receiving messages from that chain
    mapping(address => uint256) public senderNonces;
    mapping(bytes32 => Order) public orders;

    // TODO do we want to restrict tokens can be used?
    mapping(address => bool) public acceptOrdersIn; // whether to accept orders with a given origin token
    mapping(address => bool) public acceptFillsIn; // whether to accept fills with a given destination token

    // ========== Initiating Orders ========== //

    function open(OnchainOrderParams calldata orderParams) external override {
        // Validate order parameters

        if (orderParams.fillDeadline < block.timestamp + minFillDuration || orderParams.fillDeadline > block.timestamp + maxFillDuration) {
            revert("OrderBook: fill deadline invalid");
        }

        if (orderParams.amount < minOrderAmount) {
            revert("OrderBook: order amount too low");
        }

        if (!acceptOrdersIn[orderParams.originToken]) {
            revert("OrderBook: origin token not accepted");
        }

        // TODO should we store a list of valid destination chains? What about destination tokens?
        // With the fillDeadline boundaries, the worst case is that an unsupported order can't be filled and is refunded after expiry
        // However, that may not be great UX

        // Create order
        uint256 nonce = senderNonces[msg.sender]++;
        bytes32 orderId = keccak256(abi.encode(
            block.chainid,
            orderParams.destinationChainId,
            orderParams.destinationToken,
            msg.sender,
            orderParams.recipient,
            orderParams.fillDeadline,
            orderParams.amount,
            nonce
        ));

        orders[orderId] = Order({
            originChainId: uint32(block.chainid),
            destinationChainId: orderParams.destinationChainId,
            originToken: orderParams.originToken,
            destinationToken: orderParams.destinationToken,
            sender: msg.sender,
            recipient: orderParams.recipient,
            fillDeadline: uint40(orderParams.fillDeadline),
            amount: orderParams.amount,
            amountFilled: 0,
            nonce: nonce
        });

        // Transfer tokens in from the sender
        IERC20(orderParams.originToken).transferFrom(msg.sender, address(this), orderParams.amount);

        emit Open(orderId, orderParams.amount);
    }

    // ========== Refunding Orders ========== //

    // Note: this function allows anyone to trigger a refund of an order after its fill deadline has passed
    // This allows applications to gracefully handle refunds for orders that weren't completed filled for users
    function refund(bytes32 orderId) external override {
        Order storage order = orders[orderId];

        // Validate that the order can be refunded

        if (order.fillDeadline >= block.timestamp) {
            revert("OrderBook: order fill deadline not yet passed");
        }

        uint256 remaining = order.amount - order.amountFilled;

        if (remaining == 0) {
            revert("OrderBook: order already fully filled");
        }

        // Mark the order as fully filled to prevent re-entrancy and double refunds
        order.amountFilled = order.amount;

        // Transfer the remaining amount back to the sender
        IERC20(order.originToken).transfer(order.sender, remaining);

        emit Cancel(orderId, remaining);
    }


    // ========== Filling Orders ========== //
    function fill(OrderData calldata orderData, uint256 fillAmount) external override {
        // Validate fill data
        if (!acceptFillsIn[orderData.destinationToken]) {
            revert("OrderBook: destination token not accepted");
        }

        if (protocolChainId != orderData.destinationChainId) {
            revert("OrderBook: fill data destination chain ID does not match protocol chain ID");
        }

        bytes32 orderId = _getOrderId(orderData);

        // If the order is from another chain, the order will be empty
        // expect for the fillAmount from previous fills
        // However, we don't actually need to initialize anything else
        // because the rest of the information is contained in the order data
        // This is slightly less efficient for multi-fill orders, but
        // is more efficient for single-fill orders since we don't need to 
        // store any data in that case on the destination chain
        Order storage order = orders[orderId];

        if (order.fillDeadline < block.timestamp) {
            revert("OrderBook: order fill deadline passed");
        }

        // Calculate fill amount as the minimum of the filler provided amount and the remaining unfilled amount
        uint256 remaining = orderData.amount - order.amountFilled;
        if (remaining == 0) {
            revert("OrderBook: order already fully filled");
        }

        fillAmount = fillAmount > remaining ? remaining : fillAmount;

        // Update order state
        order.amountFilled += fillAmount;
    
        // Transfer tokens from the filler to the recipient
        IERC20(orderData.destinationToken).transferFrom(msg.sender, orderData.recipient, fillAmount);

        // Handle releasing the corresponding amount of origin tokens to the filler
        if (protocolChainId == orderData.originChainId) {
            // If this is a fill on the origin chain, we can immediately release the corresponding amount of origin tokens to the recipient
            // This is because the origin and destination chains are the same, so no cross-chain messaging is needed
            IERC20(order.originToken).transferFrom(address(this), orderData.recipient, fillAmount);
        } else {
            // If this is a fill on a different chain than the origin chain, we need to send a message back to the origin chain to release the corresponding amount of origin tokens to the recipient
            // TODO implement cross-chain messaging to report the fill back to the origin chain
            address messenger = chainMessenger[orderData.originChainId];
            messenger.sendFillReport(
                orderData.originChainId,
                OrderFillReport({
                    orderId: orderId,
                    filler: msg.sender,
                    fillAmount: fillAmount
                })
            );
            
        }

        emit Fill(orderId, msg.sender, fillAmount);
    }

    // ========== Receiving Fill Reports ========== //

    function reportFill(OrderFillReport calldata report) external override {
        Order storage order = orders[report.orderId];

        if (order.originChainId != protocolChainId) {
            revert("OrderBook: order origin chain ID does not match protocol chain ID");
        }

        if (msg.sender != chainMessenger[order.destinationChainId]) {
            revert("OrderBook: caller is not permitted messenger for destination chain");
        }

        // Update order state
        order.amountFilled += report.fillAmount;

        // Transfer the corresponding amount of origin tokens to the filler
        IERC20(order.originToken).transferFrom(address(this), report.filler, report.fillAmount);

        // TODO do we need an event on the origin chain when a fill is reported back?
    }


    // Order IDs are unique across chains and allow using fill data to compute the identifier
    // This is useful for tracking data against orders on both the origin and destination chains
    function _getOrderId(OrderFillData calldata fillData) internal pure returns (bytes32) {
        return keccak256(abi.encode(
            fillData.originChainId,
            fillData.destinationChainId,
            fillData.destinationToken,
            fillData.sender,
            fillData.recipient,
            fillData.fillDeadline,
            fillData.amount,
            fillData.nonce
        ));
    }




}
