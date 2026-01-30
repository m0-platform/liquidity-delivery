// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { console2 } from "../../lib/forge-std/src/Script.sol";

import { ScriptBase } from "../ScriptBase.s.sol";
import { IOrderBook } from "../../src/interfaces/IOrderBook.sol";

/// @title CancelOrder
/// @notice Script to cancel test orders on testnets
/// @dev Usage: forge script script/test/CancelOrder.s.sol --rpc-url <dest_rpc> --broadcast \
///             --sig "run(bytes32,string,bytes)" \
///             <orderId> <originChainAlias> <bridgeAdapterArgs>
/// @dev Uses Forge's multichain fork support to query OrderData from the origin chain
/// @dev Cancel authorization:
///      - Before deadline: recipient (or sender for same-chain orders) can cancel
///      - After deadline: anyone can cancel (permissionless refund)
contract CancelOrder is ScriptBase {
    /// @notice Portal interface for getting quote
    /// @dev PayloadType enum values from Portal
    uint8 constant PAYLOAD_TYPE_CANCEL_REPORT = 6;

    struct CancelConfig {
        address caller;
        address orderBook;
        bytes32 orderId;
        uint256 portalFee;
        bool isSameChain;
    }

    /// @notice Cancel an order
    /// @param orderId_ The ID of the order to cancel
    /// @param originChainAlias_ Chain alias (e.g., "sepolia") to query OrderData from
    /// @param bridgeAdapterArgs_ Optional bridge adapter args (e.g., signed Wormhole quote)
    /// @return messageId_ The cross-chain message ID (zero for same-chain cancels)
    function run(
        bytes32 orderId_,
        string calldata originChainAlias_,
        bytes calldata bridgeAdapterArgs_
    ) external returns (bytes32 messageId_) {
        // Store destination fork ID (the current --rpc-url)
        uint256 destForkId_ = vm.activeFork();

        // Query OrderData from origin chain via fork
        IOrderBook.OrderData memory orderData_ = _queryOriginOrderData(orderId_, originChainAlias_);

        // Switch back to destination fork for any reads during cancel
        vm.selectFork(destForkId_);

        // Build cancel configuration
        CancelConfig memory config_ = _buildCancelConfig(orderId_, orderData_, bridgeAdapterArgs_.length == 0);

        // Execute cancel
        messageId_ = _executeCancel(config_, orderData_, bridgeAdapterArgs_);

        // Log cancel details
        _logCancelDetails(config_, messageId_, orderData_.originChainId);
    }

    /// @notice Build cancel configuration
    function _buildCancelConfig(
        bytes32 orderId_,
        IOrderBook.OrderData memory orderData_,
        bool needsQuote_
    ) internal returns (CancelConfig memory config_) {
        config_.caller = vm.rememberKey(vm.envUint("SENDER_PRIVATE_KEY"));
        config_.orderBook = _readDeployment(block.chainid);
        config_.orderId = orderId_;
        config_.isSameChain = orderData_.originChainId == block.chainid;

        // Verify order ID matches computed ID
        bytes32 computedOrderId_ = IOrderBook(config_.orderBook).getOrderId(orderData_);
        require(orderId_ == computedOrderId_, "Order ID mismatch");

        // Determine Portal fee for cross-chain cancels
        if (!config_.isSameChain && needsQuote_) {
            address portal_ = _getPortalAddress();
            config_.portalFee = _getPortalQuote(portal_, orderData_.originChainId);
            // solhint-disable-next-line no-console
            console2.log("Portal fee (wei):", config_.portalFee);
        }
    }

    /// @notice Execute the cancel transaction
    function _executeCancel(
        CancelConfig memory config_,
        IOrderBook.OrderData memory orderData_,
        bytes calldata bridgeAdapterArgs_
    ) internal returns (bytes32 messageId_) {
        vm.startBroadcast(config_.caller);

        // Cancel order - no token approval needed since we're not transferring tokens
        if (bridgeAdapterArgs_.length > 0) {
            messageId_ = IOrderBook(config_.orderBook).cancelOrder{ value: config_.portalFee }(
                config_.orderId,
                orderData_,
                bridgeAdapterArgs_
            );
        } else {
            messageId_ = IOrderBook(config_.orderBook).cancelOrder{ value: config_.portalFee }(
                config_.orderId,
                orderData_
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

    /// @notice Get Portal address from config
    function _getPortalAddress() internal view returns (address) {
        string memory configPath_ = string.concat(vm.projectRoot(), "/config/chains.json");
        string memory config_ = vm.readFile(configPath_);
        return vm.parseJsonAddress(config_, ".portal");
    }

    /// @notice Get Portal quote for cancel report
    /// @dev Uses the Portal's quote function for Hyperlane (default adapter)
    function _getPortalQuote(address portal_, uint32 destChainId_) internal view returns (uint256) {
        // Call Portal.quote(destChainId, PayloadType.CancelReport)
        // PayloadType.CancelReport = 6
        (bool success_, bytes memory data_) = portal_.staticcall(
            abi.encodeWithSignature("quote(uint32,uint8)", destChainId_, PAYLOAD_TYPE_CANCEL_REPORT)
        );

        if (!success_ || data_.length < 32) {
            // solhint-disable-next-line no-console
            console2.log("Warning: Could not get Portal quote, using 0");
            return 0;
        }

        return abi.decode(data_, (uint256));
    }

    /// @notice Log cancel details
    function _logCancelDetails(CancelConfig memory config_, bytes32 messageId_, uint32 originChainId_) internal pure {
        // solhint-disable-next-line no-console
        console2.log("\n=== Cancel Details ===");
        // solhint-disable-next-line no-console
        console2.log("Order ID:", vm.toString(config_.orderId));
        // solhint-disable-next-line no-console
        console2.log("Caller:", config_.caller);

        if (config_.isSameChain) {
            // solhint-disable-next-line no-console
            console2.log("Cancel Type: Same-chain (immediate refund)");
        } else {
            // solhint-disable-next-line no-console
            console2.log("Cancel Type: Cross-chain");
            // solhint-disable-next-line no-console
            console2.log("Message ID:", vm.toString(messageId_));
            // solhint-disable-next-line no-console
            console2.log("Origin Chain:", originChainId_);
        }

        // solhint-disable-next-line no-console
        console2.log("=== Cancel Complete ===\n");
    }
}
