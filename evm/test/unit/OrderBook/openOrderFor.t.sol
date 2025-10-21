// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { VmSafe } from "../../../lib/forge-std/src/Vm.sol";
import { console } from "../../../lib/forge-std/src/console.sol";

import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";
import { TypeConverter } from "../../../src/libs/TypeConverter.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";


contract OpenOrderForTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the signature is invalid 
    //   [X] it reverts
    // [X] given the origin chain ID is not the current internal chain ID
    //   [X] it reverts with an InvalidOriginChain error
    // [X] given the nonce does not match the current nonce for the sender
    //   [X] it reverts with an InvalidNonce error
    // [ ] given the sender has not approved the orderbook contract for the tokenIn
    //   [ ] it reverts with an insufficient allowance error
    // [ ] given the signature is a valid standard ECDSA signature
    //   [ ] 
    // [ ] given the signature is a valid compact ECDSA signature
    // [ ] 

    IOrderBook.GaslessOrderParams internal gaslessParams;
    VmSafe.Wallet internal sender;

    function setUp() public override {
        super.setUp();

        sender = vm.createWallet("sender");
        vm.deal(sender.addr, 1 ether);
        for (uint256 i; i < TOKEN_COUNT; i++) {
            tokens[i].mint(sender.addr, MINT_AMOUNT);
        }

        gaslessParams = IOrderBook.GaslessOrderParams({
            originChainId: CHAIN_ID,
            tokenIn: params.tokenIn,
            destChainId: params.destChainId,
            tokenOut: params.tokenOut,
            amountIn: params.amountIn,
            amountOut: params.amountOut,
            sender: sender.addr,
            nonce: orderBook.getSenderNonce(sender.addr),
            recipient: sender.addr.toBytes32(),
            fillDeadline: params.fillDeadline,
            solver: params.solver 
        });

        // This is not optimal
        vm.prank(sender.addr);
        tokens[0].approve(address(orderBook), type(uint256).max);
    }

    function _signStandardECDSA(VmSafe.Wallet memory wallet_, IOrderBook.GaslessOrderParams memory params_) internal returns (bytes memory) {
        bytes32 digest = orderBook.getGaslessOrderDigest(params_);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(wallet_, digest);
        return abi.encodePacked(r, s, v);
    }

    function _signCompactECDSA(VmSafe.Wallet memory wallet_, IOrderBook.GaslessOrderParams memory params_) internal returns (bytes memory) {
        bytes32 digest = orderBook.getGaslessOrderDigest(params_);
        (bytes32 r, bytes32 vs) = vm.signCompact(wallet_, digest);
        return abi.encodePacked(r, vs);
    }

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

    // TODO: this should pass, but the EIP712 base contract that is being
    // inherited here only accepts standard 65 byte length ECDSA signatures
    // Need to make a PR to common
    function test_givenCompactECDSASignature_success() public {
        // Create the order digest and sign it
        bytes memory signature = _signCompactECDSA(sender, gaslessParams);

        // Cache the starting balance of tokenIn
        uint256 startingBalance = tokens[0].balanceOf(sender.addr);

        bytes32 orderId = orderBook.openOrderFor(gaslessParams, signature);

        // Get the order and confirm the data is set correctly
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created), "status");
        assertEq(order.version, VERSION, "version");
        assertEq(order.destChainId, gaslessParams.destChainId, "destChainId");
        assertEq(order.fillDeadline, gaslessParams.fillDeadline, "fillDeadline");
        assertEq(order.refundRequestedAt, uint40(0), "refundRequestedAt");
        assertEq(order.nonce, gaslessParams.nonce, "nonce");
        assertEq(order.tokenIn, gaslessParams.tokenIn, "tokenIn");
        assertEq(order.tokenOut, gaslessParams.tokenOut, "tokenOut");
        assertEq(order.sender, gaslessParams.sender, "sender");
        assertEq(order.recipient, gaslessParams.recipient, "recipient");
        assertEq(order.amountIn, gaslessParams.amountIn, "amountIn");
        assertEq(order.amountOut, gaslessParams.amountOut, "amountOut");
        assertEq(order.solver, gaslessParams.solver, "solver");

        // Confirm the correct amount was transferred from the sender
        assertEq(tokens[0].balanceOf(sender.addr), startingBalance - gaslessParams.amountIn);
    }

    function test_givenStandardECDSASignature_success() public {
        // Create the order digest and sign it 
        bytes memory signature = _signStandardECDSA(sender, gaslessParams);

        // Cache the starting balance of tokenIn
        uint256 startingBalance = tokens[0].balanceOf(sender.addr);

        bytes32 orderId = orderBook.openOrderFor(gaslessParams, signature);

        // Get the order and confirm the data is set correctly
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created), "status");
        assertEq(order.version, VERSION, "version");
        assertEq(order.destChainId, gaslessParams.destChainId, "destChainId");
        assertEq(order.fillDeadline, gaslessParams.fillDeadline, "fillDeadline");
        assertEq(order.refundRequestedAt, uint40(0), "refundRequestedAt");
        assertEq(order.nonce, gaslessParams.nonce, "nonce");
        assertEq(order.tokenIn, gaslessParams.tokenIn, "tokenIn");
        assertEq(order.tokenOut, gaslessParams.tokenOut, "tokenOut");
        assertEq(order.sender, gaslessParams.sender, "sender");
        assertEq(order.recipient, gaslessParams.recipient, "recipient");
        assertEq(order.amountIn, gaslessParams.amountIn, "amountIn");
        assertEq(order.amountOut, gaslessParams.amountOut, "amountOut");
        assertEq(order.solver, gaslessParams.solver, "solver");

        // Confirm the correct amount was transferred from the sender
        assertEq(tokens[0].balanceOf(sender.addr), startingBalance - gaslessParams.amountIn);
    }


}