// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { IERC20 } from "../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/interfaces/IERC20.sol";
import { console2 } from "../../lib/forge-std/src/Script.sol";

import { ScriptBase } from "../ScriptBase.s.sol";
import { IOrderBook } from "../../src/interfaces/IOrderBook.sol";

/// @title FillOrder
/// @notice Script to fill test orders on testnets with partial fill support
/// @dev Usage: forge script script/test/FillOrder.s.sol --rpc-url <dest_rpc> --broadcast \
///             --sig "run(bytes32,string,uint128,bytes32,address,bytes)" \
///             <orderId> <originChainAlias> <amountOutToFill> <originRecipient> <bridgeAdapter> <bridgeAdapterArgs>
/// @dev Uses Forge's multichain fork support to query OrderData from the origin chain
contract FillOrder is ScriptBase {
    /// @notice Portal interface for getting quote
    /// @dev PayloadType enum values from Portal
    uint8 constant PAYLOAD_TYPE_FILL_REPORT = 4;

    struct FillConfig {
        address solver;
        address orderBook;
        bytes32 orderId;
        uint128 amountOutToFill;
        bytes32 originRecipient;
        address bridgeAdapter;
        uint256 portalFee;
        bool isSameChain;
    }

    /// @notice Fill an order with the given parameters
    /// @param orderId_ The ID of the order to fill
    /// @param originChainAlias_ Chain alias (e.g., "sepolia") to query OrderData from
    /// @param amountOutToFill_ Amount of output token to provide (supports partial fills)
    /// @param originRecipient_ Address on origin chain to receive released funds (defaults to solver if zero)
    /// @param bridgeAdapter_ Bridge adapter address (zero = default adapter)
    /// @param bridgeAdapterArgs_ Optional bridge adapter args (e.g., signed Wormhole quote)
    /// @return messageId_ The cross-chain message ID (zero for same-chain fills)
    function run(
        bytes32 orderId_,
        string calldata originChainAlias_,
        uint128 amountOutToFill_,
        bytes32 originRecipient_,
        address bridgeAdapter_,
        bytes calldata bridgeAdapterArgs_
    ) external returns (bytes32 messageId_) {
        // Store destination fork ID (the current --rpc-url)
        uint256 destForkId_ = vm.activeFork();

        // Query OrderData from origin chain via fork
        IOrderBook.OrderData memory orderData_ = _queryOriginOrderData(orderId_, originChainAlias_);

        // Switch back to destination fork for any reads during fill
        vm.selectFork(destForkId_);

        // Build fill configuration
        FillConfig memory config_ = _buildFillConfig(
            orderId_,
            orderData_,
            amountOutToFill_,
            originRecipient_,
            bridgeAdapter_,
            bridgeAdapterArgs_.length == 0 && bridgeAdapter_ == address(0)
        );

        // Execute fill
        messageId_ = _executeFill(config_, orderData_, bridgeAdapterArgs_);

        // Log fill details
        _logFillDetails(config_, messageId_, orderData_.originChainId);
    }

    /// @notice Build fill configuration
    function _buildFillConfig(
        bytes32 orderId_,
        IOrderBook.OrderData memory orderData_,
        uint128 amountOutToFill_,
        bytes32 originRecipient_,
        address bridgeAdapter_,
        bool needsPortalQuote_
    ) internal returns (FillConfig memory config_) {
        config_.solver = vm.rememberKey(vm.envUint("SOLVER_PRIVATE_KEY"));
        config_.orderBook = _readDeployment(block.chainid);
        config_.orderId = orderId_;
        config_.amountOutToFill = amountOutToFill_;
        config_.bridgeAdapter = bridgeAdapter_;
        config_.isSameChain = orderData_.originChainId == block.chainid;

        // Verify order ID matches computed ID
        bytes32 computedOrderId_ = IOrderBook(config_.orderBook).getOrderId(orderData_);
        require(orderId_ == computedOrderId_, "Order ID mismatch");

        // Default origin recipient to solver if not specified
        config_.originRecipient = originRecipient_ == bytes32(0)
            ? bytes32(uint256(uint160(config_.solver)))
            : originRecipient_;

        // Determine fee for cross-chain fills
        if (!config_.isSameChain) {
            if (needsPortalQuote_) {
                // Default adapter (Hyperlane): get quote from Portal on-chain
                address portal_ = _getPortalAddress();
                config_.portalFee = _getPortalQuote(portal_, orderData_.originChainId);
                // solhint-disable-next-line no-console
                console2.log("Portal fee (wei):", config_.portalFee);
            } else {
                // Non-default adapter (e.g., Wormhole): read fee from env var set by shell script
                config_.portalFee = vm.envOr("BRIDGE_FEE", uint256(0));
                // solhint-disable-next-line no-console
                console2.log("Bridge fee from env (wei):", config_.portalFee);
            }
        }
    }

    /// @notice Execute the fill transaction
    function _executeFill(
        FillConfig memory config_,
        IOrderBook.OrderData memory orderData_,
        bytes calldata bridgeAdapterArgs_
    ) internal returns (bytes32 messageId_) {
        // Get token out address for approval
        address tokenOut_ = address(uint160(uint256(orderData_.tokenOut)));

        // Check and approve token allowance
        uint256 currentAllowance_ = IERC20(tokenOut_).allowance(config_.solver, config_.orderBook);

        vm.startBroadcast(config_.solver);

        if (currentAllowance_ < config_.amountOutToFill) {
            IERC20(tokenOut_).approve(config_.orderBook, type(uint256).max);
        }

        // Create fill params
        IOrderBook.FillParams memory fillParams_ = IOrderBook.FillParams({
            amountOutToFill: config_.amountOutToFill,
            originRecipient: config_.originRecipient,
            refundAddress: bytes32(uint256(uint160(config_.solver)))
        });

        // Fill order — select overload based on bridge adapter and args
        if (config_.bridgeAdapter != address(0)) {
            // Explicit bridge adapter (e.g., Wormhole)
            messageId_ = IOrderBook(config_.orderBook).fillOrder{ value: config_.portalFee }(
                config_.orderId,
                orderData_,
                fillParams_,
                config_.bridgeAdapter,
                bridgeAdapterArgs_
            );
        } else if (bridgeAdapterArgs_.length > 0) {
            // Default adapter with extra args
            messageId_ = IOrderBook(config_.orderBook).fillOrder{ value: config_.portalFee }(
                config_.orderId,
                orderData_,
                fillParams_,
                bridgeAdapterArgs_
            );
        } else {
            // Default adapter, no args
            messageId_ = IOrderBook(config_.orderBook).fillOrder{ value: config_.portalFee }(
                config_.orderId,
                orderData_,
                fillParams_
            );
        }

        vm.stopBroadcast();
    }

    /// @notice Query OrderData from origin chain via fork
    /// @param orderId_ The order ID to query
    /// @param originChainAlias_ Chain alias (e.g., "sepolia") configured in foundry.toml [rpc_endpoints]
    /// @return orderData_ The order data from the origin chain
    function _queryOriginOrderData(
        bytes32 orderId_,
        string calldata originChainAlias_
    ) internal returns (IOrderBook.OrderData memory orderData_) {
        // Get origin chain RPC URL from foundry.toml configuration
        string memory originRpcUrl_ = vm.rpcUrl(originChainAlias_);

        // Create fork of origin chain (read-only)
        uint256 originForkId_ = vm.createFork(originRpcUrl_);
        vm.selectFork(originForkId_);

        // Get origin chain ID and OrderBook address
        uint256 originChainId_ = block.chainid;
        address originOrderBook_ = _readDeployment(originChainId_);

        // Query OrderData from origin chain
        orderData_ = IOrderBook(originOrderBook_).getOrderData(orderId_);

        // Verify order exists (version 0 indicates non-existent order)
        require(orderData_.version != 0, "Order does not exist on origin chain");

        // solhint-disable-next-line no-console
        console2.log("Queried order from origin chain:", originChainId_);
        // solhint-disable-next-line no-console
        console2.log("Origin OrderBook:", originOrderBook_);
    }

    /// @notice Get Portal address from PORTAL_ADDRESS env var (set by shell script from chain config)
    function _getPortalAddress() internal view returns (address) {
        return vm.envAddress("PORTAL_ADDRESS");
    }

    /// @notice Get Portal quote for fill report
    /// @dev Uses the Portal's quote function for Hyperlane (default adapter)
    function _getPortalQuote(address portal_, uint32 destChainId_) internal view returns (uint256) {
        // Call Portal.quote(destChainId, PayloadType.FillReport)
        // PayloadType.FillReport = 4
        (bool success_, bytes memory data_) = portal_.staticcall(
            abi.encodeWithSignature("quote(uint32,uint8)", destChainId_, PAYLOAD_TYPE_FILL_REPORT)
        );

        if (!success_ || data_.length < 32) {
            // solhint-disable-next-line no-console
            console2.log("Warning: Could not get Portal quote, using 0");
            return 0;
        }

        return abi.decode(data_, (uint256));
    }

    /// @notice Log fill details
    function _logFillDetails(FillConfig memory config_, bytes32 messageId_, uint32 originChainId_) internal pure {
        // solhint-disable-next-line no-console
        console2.log("\n=== Fill Details ===");
        // solhint-disable-next-line no-console
        console2.log("Order ID:", vm.toString(config_.orderId));
        // solhint-disable-next-line no-console
        console2.log("Amount Out Filled:", uint256(config_.amountOutToFill));
        // solhint-disable-next-line no-console
        console2.log("Origin Recipient:", address(uint160(uint256(config_.originRecipient))));

        if (config_.isSameChain) {
            // solhint-disable-next-line no-console
            console2.log("Fill Type: Same-chain (immediate settlement)");
        } else {
            // solhint-disable-next-line no-console
            console2.log("Fill Type: Cross-chain");
            // solhint-disable-next-line no-console
            console2.log("Message ID:", vm.toString(messageId_));
            // solhint-disable-next-line no-console
            console2.log("Origin Chain:", originChainId_);
        }

        // solhint-disable-next-line no-console
        console2.log("=== Fill Complete ===\n");
    }
}
