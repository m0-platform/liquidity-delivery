// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { DeployHelpers } from "../../lib/common/script/deploy/DeployHelpers.sol";

import { ScriptBase } from "../ScriptBase.s.sol";
import { OrderBook } from "../../src/OrderBook.sol";

contract Deploy is ScriptBase, DeployHelpers {
    /// @dev Contract name used for deterministic deployment.
    string internal constant _ORDER_BOOK_CONTRACT_NAME = "OrderBook";

    function run() external {
        // TODO update to use foundry keystore
        address deployer_ = vm.rememberKey(vm.envUint("PRIVATE_KEY"));

        uint32 m0ChainId_ = uint32(vm.envUint("CHAIN_ID"));

        vm.startBroadcast(deployer_);

        // TODO use defined configuration and chain definitions to handle inputs
        (, address orderBook_) = _deployOrderBook(
            deployer_,
            vm.envAddress("ADMIN_ADDRESS"),
            m0ChainId_,
            vm.envAddress("PORTAL_ADDRESS")
        );

        vm.stopBroadcast();

        _serializeDeployment(m0ChainId_, orderBook_);
    }

    /**
     * @dev Deploys the OrderBook contract to a deterministic address using CREATE3.
     * @param deployer_ The address of the deployer.
     * @param admin_ The address to set as the admin of the contract.
     * @param chainId_ The M0 chain ID to use for the OrderBook on the chain being deployed to.
     * @param portal_ The address of the portal contract for cross-chain communication.
     */
    function _deployOrderBook(
        address deployer_,
        address admin_,
        uint32 chainId_,
        address portal_
    ) internal returns (address implementation_, address proxy_) {
        implementation_ = address(new OrderBook(chainId_, portal_));

        proxy_ = _deployCreate3TransparentProxy(
            implementation_,
            admin_,
            abi.encodeWithSelector(OrderBook.initialize.selector, admin_),
            _computeSalt(deployer_, _ORDER_BOOK_CONTRACT_NAME)
        );
    }

    function _serializeDeployment(uint32 chainId_, address orderBook_) internal {
        string memory root = "";
        vm.writeJson(vm.serializeAddress(root, "orderBook", orderBook_), _deployOutputPath(chainId_));
    }
}
