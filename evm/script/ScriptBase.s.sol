// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { Script } from "../lib/forge-std/src/Script.sol";

contract ScriptBase is Script {
    struct Deployment {
        address orderBook;
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
}
