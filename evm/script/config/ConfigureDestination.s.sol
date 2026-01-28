// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { ScriptBase } from "../ScriptBase.s.sol";
import { IOrderBook } from "../../src/interfaces/IOrderBook.sol";

/// @title ConfigureDestination
/// @notice Script to configure a destination chain as supported on an OrderBook deployment
/// @dev Usage: forge script script/config/ConfigureDestination.s.sol --rpc-url <rpc> --broadcast \
///             --sig "run(address,uint32,bool)" <orderBook> <destChainId> <isSupported>
contract ConfigureDestination is ScriptBase {
    /// @notice Configure a destination chain as supported or unsupported
    /// @param orderBook_ The OrderBook contract address
    /// @param destChainId_ The destination chain ID to configure
    /// @param isSupported_ Whether the destination should be supported
    function run(address orderBook_, uint32 destChainId_, bool isSupported_) external {
        address deployer_ = vm.rememberKey(vm.envUint("ADMIN_PRIVATE_KEY"));

        // Check current state
        bool currentlySupported_ = IOrderBook(orderBook_).isDestinationSupported(destChainId_);

        if (currentlySupported_ == isSupported_) {
            // solhint-disable-next-line no-console
            return;
        }

        vm.startBroadcast(deployer_);
        IOrderBook(orderBook_).setDestinationSupported(destChainId_, isSupported_);
        vm.stopBroadcast();
    }
}
