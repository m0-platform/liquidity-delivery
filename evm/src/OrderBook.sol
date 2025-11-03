// SPDX-License-Identifier: GPL-3.0
pragma solidity 0.8.26;

import { IERC20 } from "../lib/common/src/interfaces/IERC20.sol";
import { IERC20Extended } from "../lib/common/src/interfaces/IERC20Extended.sol";
import { AccessControlUpgradeable } from "../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/access/AccessControlUpgradeable.sol";
import { ERC712ExtendedUpgradeable } from "../lib/common/src/ERC712ExtendedUpgradeable.sol";

import { IOrderBook } from "./interfaces/IOrderBook.sol";
import { IMessenger } from "./interfaces/IMessenger.sol";
import { TypeConverter } from "./libs/TypeConverter.sol";
import { TransferHelper } from "./libs/TransferHelper.sol";

abstract contract OrderBookStorageLayout {
    /// @custom:storage-location erc7201:M0.storage.OrderBook
    struct OrderBookStorageStruct {
        // destination configuration
        mapping(uint32 destChainId => IOrderBook.Destination) destinations;
        // only store full data about origin orders
        mapping(bytes32 orderId => IOrderBook.Order) localOrders;
        // store fill amounts for both origin and destination orders
        mapping(bytes32 orderId => IOrderBook.FilledAmounts) filledAmounts;
        // track nonces for each sender to ensure unique order IDs
        mapping(address sender => uint64 nonce) senderNonces;
    }

    // keccak256(abi.encode(uint256(keccak256("M0.storage.OrderBook")) - 1)) & ~bytes32(uint256(0xff))
    bytes32 private constant _ORDER_BOOK_STORAGE_LOCATION =
        0x820bf725beb8a0ae85e433f17c2d2091cee8f490a62ab8bd0d9dd95db3dddc00;

    function _getOrderBookStorageLocation() internal pure returns (OrderBookStorageStruct storage $) {
        assembly {
            $.slot := _ORDER_BOOK_STORAGE_LOCATION
        }
    }
}

contract OrderBook is IOrderBook, OrderBookStorageLayout, AccessControlUpgradeable, ERC712ExtendedUpgradeable {
    using TypeConverter for *;
    using TransferHelper for IERC20;

    // ========== State Variables ========== //

    /// @notice Version of the limit order system
    uint16 public constant VERSION = 1;

    /// @notice the type hash used for gasless order submission
    /// @dev keccak256("GaslessOrderParams(uint16 version,address sender,uint64 nonce,uint32 originChainId,uint32 destChainId,uint32 fillDeadline,address tokenIn,bytes32 tokenOut,uint128 amountIn,uint128 amountOut,bytes32 recipient,bytes32 solver)")
    bytes32 public constant GASLESS_ORDER_TYPEHASH = 0xdcc220f897990a71a7c6f1069339af0681016bb96f2d791f2214e234d7029603;

    /// @notice the type hash used for cancel request signatures
    /// @dev keccak256("CancelRequest(bytes32 orderId)")
    bytes32 public constant CANCEL_REQUEST_TYPEHASH =
        0xb527222e97466d0fc0fe78079eb3beced1bb20e1103e09bb21df58dde41c6c92;

    /// @notice the chain ID of this chain according to the messaging network used by this contract
    uint32 public immutable chainId;

    /// @notice the messenger contract used for cross-chain communication
    /// @dev sends crosschain messages to report fills on this chain to other chains
    ///      receive crosschain messages to report fills on other chains to this chain
    address public immutable messenger;

    /* ========== Construct and Initialize ========== */

    constructor(uint32 chainId_, address messenger_) {
        chainId = chainId_;
        messenger = messenger_;
    }

    function initialize(address admin) external initializer {
        __ERC712ExtendedUpgradeable_init("M0 OrderBook");

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
    }

    /* ========== Creating Orders ========== */

    /// @inheritdoc IOrderBook
    function openOrder(OrderParams calldata orderParams_) external override returns (bytes32) {
        return _openOrder(msg.sender, orderParams_);
    }

    /// @inheritdoc IOrderBook
    function openOrderWithPermit(
        OrderParams calldata orderParams_,
        uint256 deadline_,
        uint8 v_,
        bytes32 r_,
        bytes32 s_
    ) external override returns (bytes32) {
        try
            IERC20Extended(orderParams_.tokenIn).permit(
                msg.sender,
                address(this),
                uint256(orderParams_.amountIn),
                deadline_,
                v_,
                r_,
                s_
            )
        {} catch {}
        return _openOrder(msg.sender, orderParams_);
    }

    /// @inheritdoc IOrderBook
    function openOrderWithPermit(
        OrderParams calldata orderParams_,
        uint256 deadline_,
        bytes memory permitSignature_
    ) external override returns (bytes32) {
        try
            IERC20Extended(orderParams_.tokenIn).permit(
                msg.sender,
                address(this),
                uint256(orderParams_.amountIn),
                deadline_,
                permitSignature_
            )
        {} catch {}
        return _openOrder(msg.sender, orderParams_);
    }

    /// @inheritdoc IOrderBook
    function openOrderFor(
        GaslessOrderParams calldata orderParams_,
        bytes calldata orderSignature_
    ) external override returns (bytes32) {
        return _openOrderFor(orderParams_, orderSignature_);
    }

    /// @inheritdoc IOrderBook
    function openOrderForWithPermit(
        GaslessOrderParams calldata orderParams_,
        bytes calldata orderSignature_,
        uint256 deadline_,
        uint8 v_,
        bytes32 r_,
        bytes32 s_
    ) external override returns (bytes32) {
        try
            IERC20Extended(orderParams_.tokenIn).permit(
                orderParams_.sender,
                address(this),
                uint256(orderParams_.amountIn),
                deadline_,
                v_,
                r_,
                s_
            )
        {} catch {}
        return _openOrderFor(orderParams_, orderSignature_);
    }

    /// @inheritdoc IOrderBook
    function openOrderForWithPermit(
        GaslessOrderParams calldata orderParams_,
        bytes calldata orderSignature_,
        uint256 deadline_,
        bytes memory permitSignature_
    ) external override returns (bytes32) {
        try
            IERC20Extended(orderParams_.tokenIn).permit(
                orderParams_.sender,
                address(this),
                uint256(orderParams_.amountIn),
                deadline_,
                permitSignature_
            )
        {} catch {}
        return _openOrderFor(orderParams_, orderSignature_);
    }

    function _openOrder(address sender_, OrderParams memory orderParams_) internal returns (bytes32) {
        // Validate order parameters
        if (uint256(orderParams_.fillDeadline) < block.timestamp) revert InvalidDeadline();
        if (orderParams_.amountIn == 0) revert AmountInZero();
        if (orderParams_.amountOut == 0) revert AmountOutZero();
        if (orderParams_.recipient == bytes32(0)) revert InvalidRecipient();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();

        // Destination chain must either be the current chain or a supported destination
        if (orderParams_.destChainId != chainId && !$.destinations[orderParams_.destChainId].isSupported)
            revert InvalidDestinationChain();

        // Create order
        uint64 nonce_ = $.senderNonces[sender_]++;

        bytes32 orderId_ = getOrderId(
            OrderData({
                version: VERSION, // origin contract version
                originChainId: chainId,
                sender: sender_.toBytes32(),
                nonce: nonce_,
                destChainId: orderParams_.destChainId,
                fillDeadline: uint64(orderParams_.fillDeadline),
                tokenOut: orderParams_.tokenOut,
                recipient: orderParams_.recipient,
                amountIn: orderParams_.amountIn,
                amountOut: orderParams_.amountOut,
                solver: orderParams_.solver
            })
        );

        $.localOrders[orderId_] = Order({
            version: VERSION, // origin contract version
            status: OrderStatus.Created,
            destChainId: orderParams_.destChainId,
            cancelRequestedAt: uint32(0),
            fillDeadline: orderParams_.fillDeadline,
            nonce: nonce_,
            tokenIn: orderParams_.tokenIn,
            tokenOut: orderParams_.tokenOut,
            sender: sender_,
            recipient: orderParams_.recipient,
            amountIn: orderParams_.amountIn,
            amountOut: orderParams_.amountOut,
            solver: orderParams_.solver
        });

        // Transfer tokens in from the sender, ensuring the required amount is received
        IERC20(orderParams_.tokenIn).safeTransferExactFrom(sender_, address(this), uint256(orderParams_.amountIn));

        emit OrderOpened(
            orderId_,
            orderParams_.tokenIn,
            orderParams_.amountIn,
            orderParams_.destChainId,
            orderParams_.tokenOut,
            orderParams_.amountOut,
            orderParams_.solver
        );

        return orderId_;
    }

    function _openOrderFor(
        GaslessOrderParams calldata orderParams_,
        bytes calldata signature_
    ) internal returns (bytes32) {
        // Verify signature
        if (signature_.length == 64) {
            (bytes32 r, bytes32 vs) = abi.decode(signature_, (bytes32, bytes32));
            _revertIfInvalidSignature(orderParams_.sender, getGaslessOrderDigest(orderParams_), r, vs);
        } else {
            _revertIfInvalidSignature(orderParams_.sender, getGaslessOrderDigest(orderParams_), signature_);
        }

        // Verify origin chain and sender nonce
        if (orderParams_.originChainId != chainId) revert InvalidOriginChain();
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        // Requiring a nonce in the order provides replay protection for the sender
        if (orderParams_.nonce != $.senderNonces[orderParams_.sender]) revert InvalidNonce();

        // Open order on behalf of the sender
        return
            _openOrder(
                orderParams_.sender,
                OrderParams({
                    destChainId: orderParams_.destChainId,
                    tokenIn: orderParams_.tokenIn,
                    tokenOut: orderParams_.tokenOut,
                    amountIn: orderParams_.amountIn,
                    amountOut: orderParams_.amountOut,
                    recipient: orderParams_.recipient,
                    fillDeadline: orderParams_.fillDeadline,
                    solver: orderParams_.solver
                })
            );
    }

    /* ========== Refunding Orders ========== */

    /// @inheritdoc IOrderBook
    function requestCancelOrder(bytes32 orderId_) external override {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.localOrders[orderId_];

        // Validate that the caller is the sender
        if (order.sender != msg.sender) revert NotAuthorized();

        _requestCancelOrder(orderId_);
    }

    /// @inheritdoc IOrderBook
    function requestCancelOrderFor(bytes32 orderId_, bytes calldata signature_) external override {
        // Load order to get sender
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.localOrders[orderId_];

        // Verify signature
        if (signature_.length == 64) {
            (bytes32 r, bytes32 vs) = abi.decode(signature_, (bytes32, bytes32));
            _revertIfInvalidSignature(order.sender, getCancelRequestDigest(orderId_), r, vs);
        } else {
            _revertIfInvalidSignature(order.sender, getCancelRequestDigest(orderId_), signature_);
        }

        _requestCancelOrder(orderId_);
    }

    function _requestCancelOrder(bytes32 orderId_) internal {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.localOrders[orderId_];

        // Validate that the order can be cancelled and the caller is the sender
        if (order.status != OrderStatus.Created) revert InvalidOrderStatus();
        if (uint256(order.fillDeadline) < block.timestamp) revert OrderExpired();

        // Mark the order as cancel requested
        order.status = OrderStatus.CancelRequested;

        // Set the cancelRequestedAt timestamp to the current time
        // This will allow the caller to claim a refund after the finality buffer has passed
        order.cancelRequestedAt = uint32(block.timestamp); // can't overflow until year 2106 (80 years)

        emit CancelRequested(orderId_, order.cancelRequestedAt);

        if (order.destChainId == chainId) {
            // Local orders can be immediately refunded
            _claimRefund(orderId_, order);
        }
    }

    /// @inheritdoc IOrderBook
    function claimRefund(bytes32 orderId_) external override {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.localOrders[orderId_];

        // Validate that the order can be refunded
        // If the order is local, the finality buffer is 0
        uint32 finalityBuffer_ = order.destChainId == chainId ? 0 : $.destinations[order.destChainId].finalityBuffer;
        if (order.status == OrderStatus.Created) {
            // If the order is still in Created status,
            // it can only be refunded if the fill deadline + finality buffer has passed
            if (uint256(order.fillDeadline) + finalityBuffer_ >= block.timestamp) revert FinalityPending();
        } else if (order.status == OrderStatus.CancelRequested) {
            // If the order is in CancelRequested status,
            // it can only be refunded if the refund was requested at least finality buffer ago
            if (uint256(order.cancelRequestedAt) + finalityBuffer_ >= block.timestamp) revert FinalityPending();
        } else {
            // If the order is in any other status, it cannot be refunded
            revert InvalidOrderStatus();
        }

        _claimRefund(orderId_, order);
    }

    function _claimRefund(bytes32 orderId_, Order storage order) internal {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();

        // Calculate the refund amount
        uint128 amountInRemaining_ = order.amountIn - $.filledAmounts[orderId_].amountInReleased;

        // Set the order status to completed
        order.status = OrderStatus.Completed;

        // Transfer the remaining amount back to the sender
        IERC20(order.tokenIn).safeTransfer(order.sender, uint256(amountInRemaining_));

        emit RefundClaimed(orderId_, order.sender, amountInRemaining_);
    }

    /* ========== Filling Orders ========== */

    /// @inheritdoc IOrderBook
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_
    ) external override {
        // Ensure the provided order ID matches the computed order ID from the order data
        // This check is not strictly required, but it is a useful sanity check for solvers
        // to ensure they have the order data correct
        if (orderId_ != getOrderId(orderData_)) revert OrderIdMismatch();

        // Validate fill data
        if (chainId != orderData_.destChainId) revert InvalidDestinationChain();
        if (uint256(orderData_.fillDeadline) < block.timestamp) revert OrderExpired();
        if (orderData_.version != VERSION) revert InvalidOrderVersion();
        if (fillerParams_.amountOutToFill == 0) revert FillAmountZero();

        // If the solver is specified, ensure that the caller is the designated solver
        address solver_ = orderData_.solver.toAddress();
        if (solver_ != address(0) && solver_ != msg.sender) revert NotAuthorized();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();

        // Calculate fill amount as the minimum of the filler provided amount and the remaining unfilled amount
        IOrderBook.FilledAmounts storage filledAmounts = $.filledAmounts[orderId_];
        uint128 amountOutRemaining_ = orderData_.amountOut - filledAmounts.amountOutFilled;
        if (amountOutRemaining_ == 0) revert OrderAlreadyFilled();
        bool fullFill_ = fillerParams_.amountOutToFill >= amountOutRemaining_;
        uint128 amountOutToFill_ = fullFill_ ? amountOutRemaining_ : fillerParams_.amountOutToFill;

        // Calculate the corresponding of token in to release to the filler
        uint128 amountInRemaining_ = orderData_.amountIn - filledAmounts.amountInReleased;
        uint128 amountInToRelease_ = fullFill_
            ? amountInRemaining_
            : ((uint256(orderData_.amountIn) * amountOutToFill_) / orderData_.amountOut).toUint128();

        // Update filled amounts
        filledAmounts.amountOutFilled += amountOutToFill_;
        filledAmounts.amountInReleased += amountInToRelease_;

        // If local order, release the corresponding amount of origin tokens to the filler
        if (chainId == orderData_.originChainId) {
            // If a full fill, mark the order as completed
            Order storage order = $.localOrders[orderId_];

            if (fullFill_) {
                order.status = OrderStatus.Completed;
                emit OrderCompleted(orderId_);
            }

            // If this is a fill on the origin chain, we can immediately release the token in to the filler
            // This is because the origin and destination chains are the same, so no cross-chain messaging is needed
            IERC20(order.tokenIn).safeTransferExact(
                fillerParams_.originRecipient.toAddress(),
                uint256(amountInToRelease_)
            );
        }

        // Transfer tokens from the solver to the recipient
        IERC20(orderData_.tokenOut.toAddress()).safeTransferExactFrom(
            msg.sender,
            orderData_.recipient.toAddress(),
            uint256(amountOutToFill_)
        );

        // If this is a fill on a different chain than the origin chain,
        // we need to send a message back to the origin chain to release
        // the corresponding amount of tokenIn to the solver's recipient
        if (chainId != orderData_.originChainId) {
            IMessenger(messenger).sendFillReport(
                orderData_.originChainId,
                FillReport({
                    orderId: orderId_,
                    originRecipient: fillerParams_.originRecipient,
                    amountOutFilled: amountOutToFill_,
                    amountInToRelease: amountInToRelease_
                })
            );
        }

        emit OrderFilled(orderId_, msg.sender, amountInToRelease_, amountOutToFill_);
    }

    /* ========== Receiving Fill Reports ========== */

    /// @inheritdoc IOrderBook
    function reportFill(FillReport calldata report_) external override {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.localOrders[report_.orderId];

        // Validate the fill report and sender
        if (msg.sender != messenger) revert NotAuthorized();
        if (order.status != OrderStatus.Created && order.status != OrderStatus.CancelRequested)
            revert InvalidOrderStatus();

        // Update the fill amounts for the order
        IOrderBook.FilledAmounts storage filledAmounts = $.filledAmounts[report_.orderId];
        filledAmounts.amountOutFilled += report_.amountOutFilled;
        filledAmounts.amountInReleased += report_.amountInToRelease;

        // Validate that the filled amounts do not exceed the order amounts
        if (filledAmounts.amountOutFilled > order.amountOut || filledAmounts.amountInReleased > order.amountIn)
            revert InvalidReport();

        // Mark order as completed if fully filled
        if (filledAmounts.amountOutFilled == order.amountOut) {
            order.status = OrderStatus.Completed;
            emit OrderCompleted(report_.orderId);
        }

        // Transfer the amount in to release to the recipient specified by the filler
        IERC20(order.tokenIn).safeTransferExact(
            report_.originRecipient.toAddress(),
            uint256(report_.amountInToRelease)
        );
    }

    /* ========== Admin Functions ========== */

    /// @inheritdoc IOrderBook
    function setDestinationConfig(
        uint32 destChainId_,
        bool isSupported_,
        uint32 finalityBuffer_
    ) external override onlyRole(DEFAULT_ADMIN_ROLE) {
        if (isSupported_ && finalityBuffer_ == 0) revert InvalidFinalityBuffer();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        $.destinations[destChainId_] = Destination({ isSupported: isSupported_, finalityBuffer: finalityBuffer_ });
    }

    /* ========== View Functions ========== */

    // Order IDs are unique across chains and allow using fill data to compute the identifier
    // This is useful for tracking data against orders on both the origin and destination chains
    function getOrderId(OrderData memory orderData_) public pure override returns (bytes32) {
        return
            keccak256(
                abi.encodePacked(
                    orderData_.version,
                    orderData_.sender,
                    orderData_.nonce,
                    orderData_.originChainId,
                    orderData_.destChainId,
                    orderData_.fillDeadline,
                    orderData_.tokenOut,
                    orderData_.amountIn,
                    orderData_.amountOut,
                    orderData_.recipient,
                    orderData_.solver
                )
            );
    }

    function getOrder(bytes32 orderId_) external view override returns (Order memory) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.localOrders[orderId_];
    }

    function getFilledAmounts(bytes32 orderId_) external view override returns (FilledAmounts memory) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.filledAmounts[orderId_];
    }

    function getSenderNonce(address sender_) external view override returns (uint64) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.senderNonces[sender_];
    }

    function isDestinationSupported(uint32 destChainId_) external view override returns (bool) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.destinations[destChainId_].isSupported;
    }

    function getDestinationFinalityBuffer(uint32 destChainId_) external view override returns (uint32) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.destinations[destChainId_].finalityBuffer;
    }

    /* ========== EIP-712 Digest Functions ========== */

    /// @inheritdoc IOrderBook
    function getGaslessOrderDigest(GaslessOrderParams memory orderParams_) public view override returns (bytes32) {
        return
            _getDigest(
                keccak256(
                    abi.encode(
                        GASLESS_ORDER_TYPEHASH,
                        orderParams_.version,
                        orderParams_.sender,
                        orderParams_.nonce,
                        orderParams_.originChainId,
                        orderParams_.destChainId,
                        orderParams_.fillDeadline,
                        orderParams_.tokenIn,
                        orderParams_.tokenOut,
                        orderParams_.amountIn,
                        orderParams_.amountOut,
                        orderParams_.recipient,
                        orderParams_.solver
                    )
                )
            );
    }

    /// @inheritdoc IOrderBook
    function getCancelRequestDigest(bytes32 orderId_) public view override returns (bytes32) {
        return _getDigest(keccak256(abi.encode(CANCEL_REQUEST_TYPEHASH, orderId_)));
    }
}
