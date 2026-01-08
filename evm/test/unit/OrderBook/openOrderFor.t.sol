// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { VmSafe } from "../../../lib/forge-std/src/Vm.sol";
import { console } from "../../../lib/forge-std/src/console.sol";
import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";
import { PausableUpgradeable } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/utils/PausableUpgradeable.sol";

import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";

contract OpenOrderForTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the contract is paused
    //    [X] it reverts with an EnforcedPause error
    // [X] given the signature is invalid
    //   [X] it reverts
    // [X] given the origin chain ID is not the current internal chain ID
    //   [X] it reverts with an InvalidOriginChain error
    // [X] given the nonce does not match the current nonce for the sender
    //   [X] it reverts with an InvalidNonce error
    // [X] given the sender has not approved the orderbook contract for the tokenIn
    //   [X] it reverts with an insufficient allowance error
    // [X] given the order version does not match the current version of the contract
    //   [X] it reverts with an InvalidOrderVersion error
    // [X] given the signature is a valid standard ECDSA signature
    //   [X] it creates the order successfully
    //   [X] it transfers the amount in from the "sender" to the orderbook contract
    //   [X] it emits an OrderOpened event
    // [X] given the signature is a valid compact ECDSA signature
    //   [X] it creates the order successfully
    //   [X] it transfers the amount in from the "sender" to the orderbook
    //   [X] it emits an OrderOpened event

    IOrderBook.GaslessOrderParams internal gaslessParams;
    VmSafe.Wallet internal sender;

    function setUp() public override {
        super.setUp();

        sender = vm.createWallet("sender");
        vm.deal(sender.addr, 1 ether);
        tokenIn.mint(sender.addr, MINT_AMOUNT * (10 ** tokenIn.decimals()));

        gaslessParams = IOrderBook.GaslessOrderParams({
            version: VERSION,
            sender: sender.addr,
            nonce: orderBook.getSenderNonce(sender.addr),
            originChainId: CHAIN_ID,
            destChainId: params.destChainId,
            fillDeadline: params.fillDeadline,
            tokenIn: params.tokenIn,
            tokenOut: params.tokenOut,
            amountIn: params.amountIn,
            amountOut: params.amountOut,
            recipient: sender.addr.toBytes32(),
            solver: params.solver
        });

        // This is not optimal
        vm.prank(sender.addr);
        tokenIn.approve(address(orderBook), type(uint256).max);
    }

    function _signStandardECDSA(
        VmSafe.Wallet memory wallet_,
        IOrderBook.GaslessOrderParams memory params_
    ) internal returns (bytes memory) {
        bytes32 digest = orderBook.getGaslessOrderDigest(params_);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(wallet_, digest);
        return abi.encodePacked(r, s, v);
    }

    function _signCompactECDSA(
        VmSafe.Wallet memory wallet_,
        IOrderBook.GaslessOrderParams memory params_
    ) internal returns (bytes memory) {
        bytes32 digest = orderBook.getGaslessOrderDigest(params_);
        (bytes32 r, bytes32 vs) = vm.signCompact(wallet_, digest);
        return abi.encodePacked(r, vs);
    }

    function _getOrderIdFromGaslessParams(
        IOrderBook.GaslessOrderParams memory params_
    ) internal view returns (bytes32) {
        return
            _getOrderIdFromParams(
                params_.sender,
                params_.nonce,
                IOrderBook.OrderParams({
                    destChainId: params_.destChainId,
                    fillDeadline: params_.fillDeadline,
                    tokenIn: params_.tokenIn,
                    tokenOut: params_.tokenOut,
                    amountIn: params_.amountIn,
                    amountOut: params_.amountOut,
                    recipient: params_.recipient,
                    solver: params_.solver
                })
            );
    }

    /* ========== Tests ========== */

    function test_givenSignatureNotFromSender_reverts() public {
        // Create a different wallet
        VmSafe.Wallet memory notSender = vm.createWallet("not-sender");

        // Get the params digest and sign it from the wrong address
        bytes32 digest = orderBook.getGaslessOrderDigest(gaslessParams);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(notSender, digest);
        bytes memory signature = abi.encodePacked(r, s, v);

        // Try to create the order
        // Expect it to revert with an invalid signature
        vm.expectRevert();
        orderBook.openOrderFor(gaslessParams, signature);
    }

    function test_givenOriginChainIdInvalid_reverts() public {
        // Set the chain ID to a different value than the current one
        gaslessParams.originChainId = DEST_CHAIN_ID;

        // Get the digest and sign it with the sender
        bytes memory signature = _signStandardECDSA(sender, gaslessParams);

        // Create the order
        // Expect revert due to invalid origin id
        vm.expectRevert(IOrderBook.InvalidOriginChain.selector);
        orderBook.openOrderFor(gaslessParams, signature);
    }

    function test_invalidNonce_reverts() public {
        // Set the nonce to a value that hasn't been reached yet
        gaslessParams.nonce = 5;

        // Try to sign and submit the order
        bytes memory signature = _signStandardECDSA(sender, gaslessParams);

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidNonce.selector));
        orderBook.openOrderFor(gaslessParams, signature);
    }

    function test_invalidNonce2_reverts() public {
        // Create a normal order from the sender
        vm.prank(sender.addr);
        orderBook.openOrder(params);

        // Try to use the same nonce for a gasless order
        bytes memory signature = _signStandardECDSA(sender, gaslessParams);

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidNonce.selector));
        orderBook.openOrderFor(gaslessParams, signature);
    }

    function test_givenWrongOrderVersion_reverts() public {
        // Set the version to an invalid value
        gaslessParams.version = VERSION + 1;

        // Try to sign and submit the order
        bytes memory signature = _signStandardECDSA(sender, gaslessParams);

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderVersion.selector));
        orderBook.openOrderFor(gaslessParams, signature);
    }

    function test_noApproval_reverts() public {
        vm.prank(sender.addr);
        tokenIn.approve(address(orderBook), 0);

        bytes memory signature = _signStandardECDSA(sender, gaslessParams);
        vm.expectRevert();
        orderBook.openOrderFor(gaslessParams, signature);
    }

    function test_givenCompactECDSASignature_success() public {
        // Create the order digest and sign it
        bytes memory signature = _signCompactECDSA(sender, gaslessParams);

        // Cache the starting balance of tokenIn
        uint256 startingBalance = tokenIn.balanceOf(sender.addr);

        bytes32 expOrderId = _getOrderIdFromGaslessParams(gaslessParams);

        vm.expectEmit(true, true, true, true);
        emit IOrderBook.OrderOpened(
            expOrderId,
            sender.addr,
            gaslessParams.tokenIn,
            gaslessParams.amountIn,
            gaslessParams.destChainId,
            gaslessParams.tokenOut,
            gaslessParams.amountOut,
            gaslessParams.solver
        );
        bytes32 orderId = orderBook.openOrderFor(gaslessParams, signature);

        // Get the order and confirm the data is set correctly
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        assertEq(orderId, expOrderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created), "status");
        assertEq(order.version, VERSION, "version");
        assertEq(order.destChainId, gaslessParams.destChainId, "destChainId");
        assertEq(order.fillDeadline, gaslessParams.fillDeadline, "fillDeadline");
        assertEq(order.createdAt, uint32(block.timestamp), "createdAt");
        assertEq(order.nonce, gaslessParams.nonce, "nonce");
        assertEq(order.tokenIn, gaslessParams.tokenIn, "tokenIn");
        assertEq(order.tokenOut, gaslessParams.tokenOut, "tokenOut");
        assertEq(order.sender, gaslessParams.sender, "sender");
        assertEq(order.recipient, gaslessParams.recipient, "recipient");
        assertEq(order.amountIn, gaslessParams.amountIn, "amountIn");
        assertEq(order.amountOut, gaslessParams.amountOut, "amountOut");
        assertEq(order.solver, gaslessParams.solver, "solver");

        // Confirm the correct amount was transferred from the sender
        assertEq(tokenIn.balanceOf(sender.addr), startingBalance - gaslessParams.amountIn);
    }

    function test_givenStandardECDSASignature_success() public {
        // Create the order digest and sign it
        bytes memory signature = _signStandardECDSA(sender, gaslessParams);

        // Cache the starting balance of tokenIn
        uint256 startingBalance = tokenIn.balanceOf(sender.addr);

        bytes32 expOrderId = _getOrderIdFromGaslessParams(gaslessParams);

        vm.expectEmit(true, true, true, true);
        emit IOrderBook.OrderOpened(
            expOrderId,
            sender.addr,
            gaslessParams.tokenIn,
            gaslessParams.amountIn,
            gaslessParams.destChainId,
            gaslessParams.tokenOut,
            gaslessParams.amountOut,
            gaslessParams.solver
        );
        bytes32 orderId = orderBook.openOrderFor(gaslessParams, signature);

        // Get the order and confirm the data is set correctly
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        assertEq(orderId, expOrderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created), "status");
        assertEq(order.version, VERSION, "version");
        assertEq(order.destChainId, gaslessParams.destChainId, "destChainId");
        assertEq(order.fillDeadline, gaslessParams.fillDeadline, "fillDeadline");
        assertEq(order.createdAt, uint32(block.timestamp), "createdAt");
        assertEq(order.nonce, gaslessParams.nonce, "nonce");
        assertEq(order.tokenIn, gaslessParams.tokenIn, "tokenIn");
        assertEq(order.tokenOut, gaslessParams.tokenOut, "tokenOut");
        assertEq(order.sender, gaslessParams.sender, "sender");
        assertEq(order.recipient, gaslessParams.recipient, "recipient");
        assertEq(order.amountIn, gaslessParams.amountIn, "amountIn");
        assertEq(order.amountOut, gaslessParams.amountOut, "amountOut");
        assertEq(order.solver, gaslessParams.solver, "solver");

        // Confirm the correct amount was transferred from the sender
        assertEq(tokenIn.balanceOf(sender.addr), startingBalance - gaslessParams.amountIn);
    }

    function test_whenPaused_reverts() public {
        bytes memory signature = _signStandardECDSA(sender, gaslessParams);

        vm.prank(pauser);
        orderBook.pause();

        vm.expectRevert(abi.encodeWithSelector(PausableUpgradeable.EnforcedPause.selector));
        orderBook.openOrderFor(gaslessParams, signature);
    }

    function test_openOrderForWithPermit_whenPaused_reverts() public {
        bytes memory orderSignature = _signStandardECDSA(sender, gaslessParams);

        vm.prank(pauser);
        orderBook.pause();

        vm.expectRevert(abi.encodeWithSelector(PausableUpgradeable.EnforcedPause.selector));
        orderBook.openOrderForWithPermit(
            gaslessParams,
            orderSignature,
            block.timestamp + 1 hours,
            0,
            bytes32(0),
            bytes32(0)
        );
    }
}
