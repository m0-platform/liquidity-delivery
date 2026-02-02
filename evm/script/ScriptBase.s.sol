// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { Script, stdJson, console2 } from "../lib/forge-std/src/Script.sol";
import { ERC1967Utils } from "../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Utils.sol";

contract ScriptBase is Script {
    using stdJson for string;

    struct Deployment {
        address implementation;
        address orderBook;
        address proxyAdmin;
        uint256 upgradedAt;
    }

    function _deployOutputPath(uint256 chainId_) internal view returns (string memory) {
        return string.concat(vm.projectRoot(), "/deployments/", vm.toString(chainId_), ".json");
    }

    function _readDeployment(uint256 chainId_) internal view returns (address orderBook_) {
        if (!vm.isFile(_deployOutputPath(chainId_))) {
            revert("Deployment artifacts not found");
        }

        bytes memory data = vm.parseJson(vm.readFile(_deployOutputPath(chainId_)));
        Deployment memory deployment_ = abi.decode(data, (Deployment));
        return deployment_.orderBook;
    }

    function _readKey(string memory parentNode_, string memory key_) internal pure returns (string memory) {
        return string.concat(parentNode_, key_);
    }

    /// @notice Serialize the upgrade information to the deployment file (or console in dry-run mode)
    function _serializeDeployment(
        uint256 chainId_,
        address proxy_,
        address implementation_,
        address proxyAdmin_
    ) internal {
        string memory root_ = "";
        root_.serialize("implementation", implementation_);
        root_.serialize("orderBook", proxy_);
        root_.serialize("proxyAdmin", proxyAdmin_);
        string memory output = root_.serialize("upgradedAt", block.timestamp);

        if (vm.envOr("DRY_RUN", false)) {
            console2.log("\n=== DRY RUN - Deployment JSON (not written to file) ===");
            console2.log(output);
            console2.log("=== End Deployment JSON ===\n");
        } else {
            vm.writeJson(output, _deployOutputPath(chainId_));
        }
    }

    /// @notice Get the ProxyAdmin address from the proxy's ERC-1967 admin slot
    /// @param proxy_ The proxy contract address
    /// @return The ProxyAdmin contract address
    function _getProxyAdmin(address proxy_) internal view returns (address) {
        // Read the admin slot (ERC-1967)
        // bytes32 ADMIN_SLOT = 0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103
        bytes32 adminSlot_ = vm.load(proxy_, ERC1967Utils.ADMIN_SLOT);
        return address(uint160(uint256(adminSlot_)));
    }
}
