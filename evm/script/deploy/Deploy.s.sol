// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { DeployHelpers } from "../../lib/common/script/deploy/DeployHelpers.sol";

import { ScriptBase } from "../ScriptBase.s.sol";
import { OrderBook } from "../../src/OrderBook.sol";

contract Deploy is ScriptBase, DeployHelpers {
    /// @dev Contract name used for deterministic deployment.
    string internal constant _ORDER_BOOK_CONTRACT_NAME = "OrderBook";

    function run() external {
        address deployer_ = vm.rememberKey(vm.envUint("DEPLOYER_PRIVATE_KEY"));

        vm.startBroadcast(deployer_);

        (address implementation_, address proxy_) = _deployOrderBook(
            deployer_,
            vm.envAddress("ADMIN_ADDRESS"),
            vm.envAddress("PAUSER_ADDRESS"),
            vm.envAddress("PORTAL_ADDRESS")
        );

        vm.stopBroadcast();

        _serializeDeployment(block.chainid, proxy_, implementation_, _getProxyAdmin(proxy_));
    }

    /**
     * @dev Deploys the OrderBook contract to a deterministic address using CREATE3.
     * @param deployer_ The address of the deployer.
     * @param admin_ The address to set as the admin of the contract.
     * @param portal_ The address of the portal contract for cross-chain communication.
     */
    function _deployOrderBook(
        address deployer_,
        address admin_,
        address pauser_,
        address portal_
    ) internal returns (address implementation_, address proxy_) {
        implementation_ = address(new OrderBook(portal_));

        proxy_ = _deployCreate3TransparentProxy(
            implementation_,
            admin_,
            abi.encodeWithSelector(OrderBook.initialize.selector, admin_, pauser_),
            _computeSalt(deployer_, _ORDER_BOOK_CONTRACT_NAME)
        );
    }
}
