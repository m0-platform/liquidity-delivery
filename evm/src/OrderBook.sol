// SPDX-License-Identifier: GPL-3.0
pragma solidity 0.8.26;

import { IERC20 } from "../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/interfaces/IERC20.sol";
import { IERC20Extended } from "../lib/common/src/interfaces/IERC20Extended.sol";
import { AccessControlUpgradeable } from "../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/access/AccessControlUpgradeable.sol";
import { ERC712ExtendedUpgradeable } from "../lib/common/src/ERC712ExtendedUpgradeable.sol";
import { TypeConverter } from "../lib/common/src/libs/TypeConverter.sol";
import { SafeERC20 } from "./libs/SafeERC20.sol";

import { IOrderBook } from "./interfaces/IOrderBook.sol";
import { IMessenger } from "./interfaces/IMessenger.sol";

abstract contract OrderBookStorageLayout {
    /// @custom:storage-location erc7201:M0.storage.OrderBook
    struct OrderBookStorageStruct {
        // supported destination chains
        mapping(uint32 destChainId => bool isSupported) supportedDestinations;
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
    using SafeERC20 for IERC20;

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
        if (orderParams_.destChainId != chainId && !isDestinationSupported(orderParams_.destChainId))
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
                tokenIn: orderParams_.tokenIn.toBytes32(),
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
            sender_,
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
    function cancelOrder(bytes32 orderId_, OrderData calldata orderData_, bytes memory messageData_) external override {
        if (orderId_ != getOrderId(orderData_)) revert OrderIdMismatch();
        // if (orderData_.version != VERSION) revert InvalidOrderVersion();
        if (orderData_.sender.toAddress() != msg.sender) revert NotAuthorized();

        // TODO: replace with status check
        uint128 amountOutRemaining_ = orderData_.amountOut -
            _getOrderBookStorageLocation().filledAmounts[orderId_].amountOutFilled;
        if (amountOutRemaining_ == 0) revert OrderAlreadyFilled();

        _cancelOrder(orderId_, orderData_, messageData_);
    }

    /// @inheritdoc IOrderBook
    function cancelOrderFor(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes memory messageData_,
        bytes calldata signature_
    ) external override {
        if (orderId_ != getOrderId(orderData_)) revert OrderIdMismatch();
        // if (orderData_.version != VERSION) revert InvalidOrderVersion();

        // Verify signature
        if (signature_.length == 64) {
            (bytes32 r, bytes32 vs) = abi.decode(signature_, (bytes32, bytes32));
            _revertIfInvalidSignature(orderData_.sender.toAddress(), getCancelRequestDigest(orderId_), r, vs);
        } else {
            _revertIfInvalidSignature(orderData_.sender.toAddress(), getCancelRequestDigest(orderId_), signature_);
        }

        _cancelOrder(orderId_, orderData_, messageData_);
    }

    function _cancelOrder(bytes32 orderId_, OrderData calldata orderData_, bytes memory messageData_) internal {
        if (orderData_.destChainId == chainId) {
            // Local orders can be immediately refunded
            Order storage order = _getOrderBookStorageLocation().localOrders[orderId_];

            if (order.status != OrderStatus.Created) revert InvalidOrderStatus();

            _claimRefund(orderId_, order);
        } else {
            IMessenger(messenger).sendCancelReport(
                orderData_.originChainId,
                CancelReport({ orderId: orderId_ }),
                messageData_
            );
        }
    }

    function _claimRefund(bytes32 orderId_, Order storage order) internal {
        // Calculate the refund amount
        uint128 amountInRemaining_ = order.amountIn -
            _getOrderBookStorageLocation().filledAmounts[orderId_].amountInReleased;

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
        _fillOrder(orderId_, orderData_, fillerParams_, new bytes(0));
    }

    /// @inheritdoc IOrderBook
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_,
        bytes calldata messageData_
    ) external override {
        _fillOrder(orderId_, orderData_, fillerParams_, messageData_);
    }

    function _fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_,
        bytes memory messageData_
    ) internal {
        // Ensure the provided order ID matches the computed order ID from the order data
        // This check is not strictly required, but it is a useful sanity check for solvers
        // to ensure they have the order data correct
        if (orderId_ != getOrderId(orderData_)) revert OrderIdMismatch();

        // Validate fill data
        if (chainId != orderData_.destChainId) revert InvalidDestinationChain();
        if (orderData_.fillDeadline < block.timestamp) revert OrderExpired();
        if (orderData_.version != VERSION) revert InvalidOrderVersion();
        if (fillerParams_.amountOutToFill == 0) revert FillAmountZero();

        // If the solver is specified, ensure that the caller is the designated solver
        address solver_ = orderData_.solver.toAddress();
        if (solver_ != address(0) && solver_ != msg.sender) revert NotAuthorized();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();

        // Calculate fill amount as the minimum of the filler provided amount and the remaining unfilled amount
        IOrderBook.FilledAmounts storage filledAmounts = $.filledAmounts[orderId_];

        // TODO: replace with status check
        uint128 amountOutRemaining_ = orderData_.amountOut - filledAmounts.amountOutFilled;
        if (amountOutRemaining_ == 0) revert OrderAlreadyFilled();

        bool fullFill_ = fillerParams_.amountOutToFill >= amountOutRemaining_;
        uint128 amountOutToFill_ = fullFill_ ? amountOutRemaining_ : fillerParams_.amountOutToFill;

        // Calculate the corresponding of token in to release to the filler
        uint128 amountInToRelease_ = fullFill_
            ? orderData_.amountIn - filledAmounts.amountInReleased // remaining amount
            : ((uint256(orderData_.amountIn) * amountOutToFill_) / orderData_.amountOut).toUint128();

        // Transfer tokens from the solver to the recipient
        IERC20(orderData_.tokenOut.toAddress()).safeTransferExactFrom(
            msg.sender,
            orderData_.recipient.toAddress(),
            uint256(amountOutToFill_)
        );

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
        } else {
            // If this is a fill on a different chain than the origin chain,
            // we need to send a message back to the origin chain to release
            // the corresponding amount of tokenIn to the solver's recipient
            IMessenger(messenger).sendFillReport(
                orderData_.originChainId,
                FillReport({
                    orderId: orderId_,
                    originRecipient: fillerParams_.originRecipient,
                    amountOutFilled: amountOutToFill_,
                    amountInToRelease: amountInToRelease_,
                    tokenIn: orderData_.tokenIn
                }),
                messageData_
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
        if (report_.tokenIn != order.tokenIn.toBytes32()) revert InvalidReport();

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
        // We do not check fee on transfer here to avoid potential reverts on reported fills
        IERC20(order.tokenIn).safeTransfer(report_.originRecipient.toAddress(), uint256(report_.amountInToRelease));
    }

    /// @inheritdoc IOrderBook
    function reportCancel(CancelReport calldata report_) external override {
        Order storage order = _getOrderBookStorageLocation().localOrders[report_.orderId];

        // Validate the cancel report and sender
        if (msg.sender != messenger) revert NotAuthorized();
        if (order.status != OrderStatus.Created) revert InvalidOrderStatus();

        _claimRefund(report_.orderId, order);
    }

    /* ========== Admin Functions ========== */

    /// @inheritdoc IOrderBook
    function setDestinationSupported(
        uint32 destChainId_,
        bool isSupported_
    ) external override onlyRole(DEFAULT_ADMIN_ROLE) {
        if (destChainId_ == chainId) revert InvalidDestinationChain();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        bool isSupported = $.supportedDestinations[destChainId_];

        // Don't update if the value is the same
        if (isSupported == isSupported_) return;

        $.supportedDestinations[destChainId_] = isSupported_;

        emit DestinationSupportUpdated(destChainId_, isSupported_);
    }

    /* ========== View Functions ========== */

    /// @inheritdoc IOrderBook
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
                    orderData_.tokenIn,
                    orderData_.tokenOut,
                    orderData_.amountIn,
                    orderData_.amountOut,
                    orderData_.recipient,
                    orderData_.solver
                )
            );
    }

    /// @inheritdoc IOrderBook
    function getOrder(bytes32 orderId_) external view override returns (Order memory) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.localOrders[orderId_];
    }

    /// @inheritdoc IOrderBook
    function getFilledAmounts(bytes32 orderId_) external view override returns (FilledAmounts memory) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.filledAmounts[orderId_];
    }

    /// @inheritdoc IOrderBook
    function getSenderNonce(address sender_) external view override returns (uint64) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.senderNonces[sender_];
    }

    /// @inheritdoc IOrderBook
    function isDestinationSupported(uint32 destChainId_) public view override returns (bool) {
        return _getOrderBookStorageLocation().supportedDestinations[destChainId_];
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
