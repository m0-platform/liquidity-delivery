// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { Script } from "../../lib/forge-std/src/Script.sol";
import { console } from "../../lib/forge-std/src/console.sol";
import { Create2 } from "@openzeppelin/contracts/utils/Create2.sol";

import { OrderBook } from "../../src/OrderBook.sol";
import { MockERC20 } from "../../test/mock/MockERC20.t.sol";
import { MockMessenger } from "../../test/mock/MockMessenger.t.sol";

/**
 * @title DeployLocal
 * @notice Deployment script for local Docker development environment
 * @dev Deploys OrderBook and mock ERC20 tokens, mints to test user
 *
 * Environment variables:
 *   CHAIN_ID - The chain ID for this deployment
 *   DEST_CHAIN_IDS - Comma-separated list of destination chain IDs for cross-chain config
 *   SOLVER_ADDRESS - Address of the solver/admin
 *   USER_ADDRESS - Address of the test user to receive tokens
 */
contract DeployLocal is Script {
    // Anvil's default funded account (account 0)
    uint256 constant ANVIL_PRIVATE_KEY = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;

    // Fixed salts for deterministic addresses via CREATE2
    bytes32 constant USDC_SALT = keccak256("LIQUIDITY_DELIVERY_USDC_V1");
    bytes32 constant USDT_SALT = keccak256("LIQUIDITY_DELIVERY_USDT_V1");
    bytes32 constant MESSENGER_SALT = keccak256("LIQUIDITY_DELIVERY_MESSENGER_V1");

    function run() external {
        uint32 chainId = uint32(vm.envUint("CHAIN_ID"));
        string memory destChainIdsStr = vm.envString("DEST_CHAIN_IDS");
        address solverAddress = vm.envAddress("SOLVER_ADDRESS");
        address userAddress = vm.envAddress("USER_ADDRESS");

        // Parse comma-separated destination chain IDs
        uint32[] memory destChainIds = parseChainIds(destChainIdsStr);

        vm.startBroadcast(ANVIL_PRIVATE_KEY);

        // Deployer address (Anvil account 0)
        address deployer = vm.addr(ANVIL_PRIVATE_KEY);

        // Deploy MockMessenger with CREATE2 for deterministic address
        MockMessenger messenger = new MockMessenger{salt: MESSENGER_SALT}();
        console.log("MockMessenger deployed at:", address(messenger));

        // Deploy OrderBook with messenger address
        OrderBook orderBook = new OrderBook(chainId, address(messenger));

        // Initialize with deployer as admin first so we can configure
        orderBook.initialize(deployer);

        // Configure MockMessenger to point to OrderBook
        messenger.setOrderBook(address(orderBook));

        // Configure all destination chains (before transferring admin)
        for (uint256 i = 0; i < destChainIds.length; i++) {
            orderBook.setDestinationConfig(destChainIds[i], true, 10);
            console.log("Configured destination chain:", destChainIds[i]);
        }

        console.log("OrderBook deployed at:", address(orderBook));

        // Deploy mock tokens with CREATE2 for deterministic addresses
        // These addresses will be the same regardless of deployment order or script changes
        MockERC20 usdc = new MockERC20{salt: USDC_SALT}("USD Coin", "USDC", 6);
        MockERC20 usdt = new MockERC20{salt: USDT_SALT}("Tether USD", "USDT", 6);

        console.log("USDC deployed at:", address(usdc));
        console.log("USDT deployed at:", address(usdt));

        // Mint tokens to test user (1000 tokens each with 6 decimals)
        uint256 mintAmount = 1000 * 10 ** 6;
        usdc.mint(userAddress, mintAmount);
        usdt.mint(userAddress, mintAmount);

        // Also mint to solver for filling orders
        usdc.mint(solverAddress, mintAmount);
        usdt.mint(solverAddress, mintAmount);

        console.log("Minted", mintAmount, "USDC and USDT to user:", userAddress);
        console.log("Minted", mintAmount, "USDC and USDT to solver:", solverAddress);

        // Grant admin role to solver
        orderBook.grantRole(orderBook.DEFAULT_ADMIN_ROLE(), solverAddress);

        // Renounce deployer's admin role (solver is now the only admin)
        orderBook.renounceRole(orderBook.DEFAULT_ADMIN_ROLE(), deployer);

        vm.stopBroadcast();
    }

    /// @notice Parse a comma-separated string of chain IDs into an array
    function parseChainIds(string memory str) internal pure returns (uint32[] memory) {
        // Count commas to determine array size
        bytes memory strBytes = bytes(str);
        uint256 count = 1;
        for (uint256 i = 0; i < strBytes.length; i++) {
            if (strBytes[i] == ",") {
                count++;
            }
        }

        uint32[] memory result = new uint32[](count);
        uint256 resultIndex = 0;
        uint256 start = 0;

        for (uint256 i = 0; i <= strBytes.length; i++) {
            if (i == strBytes.length || strBytes[i] == ",") {
                // Extract substring and convert to uint32
                bytes memory numBytes = new bytes(i - start);
                for (uint256 j = start; j < i; j++) {
                    numBytes[j - start] = strBytes[j];
                }
                result[resultIndex] = uint32(parseUint(string(numBytes)));
                resultIndex++;
                start = i + 1;
            }
        }

        return result;
    }

    /// @notice Parse a string to uint
    function parseUint(string memory str) internal pure returns (uint256) {
        bytes memory strBytes = bytes(str);
        uint256 result = 0;
        for (uint256 i = 0; i < strBytes.length; i++) {
            uint8 c = uint8(strBytes[i]);
            if (c >= 48 && c <= 57) {
                result = result * 10 + (c - 48);
            }
        }
        return result;
    }

    /// @notice Compute deterministic contract addresses without deploying
    /// @dev Useful for pre-computing addresses for config files
    /// Run with: forge script DeployLocal --sig "computeAddresses()"
    function computeAddresses() external view {
        address deployer = vm.addr(ANVIL_PRIVATE_KEY);

        bytes memory messengerBytecode = type(MockMessenger).creationCode;
        bytes memory usdcBytecode = abi.encodePacked(
            type(MockERC20).creationCode,
            abi.encode("USD Coin", "USDC", uint8(6))
        );
        bytes memory usdtBytecode = abi.encodePacked(
            type(MockERC20).creationCode,
            abi.encode("Tether USD", "USDT", uint8(6))
        );

        address messengerAddress = Create2.computeAddress(MESSENGER_SALT, keccak256(messengerBytecode), deployer);
        address usdcAddress = Create2.computeAddress(USDC_SALT, keccak256(usdcBytecode), deployer);
        address usdtAddress = Create2.computeAddress(USDT_SALT, keccak256(usdtBytecode), deployer);

        console.log("Deployer:", deployer);
        console.log("MockMessenger will be deployed at:", messengerAddress);
        console.log("USDC will be deployed at:", usdcAddress);
        console.log("USDT will be deployed at:", usdtAddress);
    }
}
