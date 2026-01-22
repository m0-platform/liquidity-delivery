// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { Test } from "../../../lib/forge-std/src/Test.sol";
import { ERC1967Proxy } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { Initializable } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/proxy/utils/Initializable.sol";

import { OrderBook, IOrderBook } from "../../../src/OrderBook.sol";
import { MockPortalV2 } from "../../mock/MockPortalV2.t.sol";

contract InitializationTest is Test {
    // [X] constructor with zero portal address
    //   [X] it reverts with ZeroPortal error
    // [X] given the implementation contract is deployed directly
    //   [X] calling initialize reverts with InvalidInitialization
    // [X] given the proxy is deployed and initialized
    //   [X] admin has DEFAULT_ADMIN_ROLE
    //   [X] pauser has PAUSER_ROLE
    //   [X] portal is set correctly
    //   [X] ERC712 domain name is "M0 OrderBook"
    //   [X] calling initialize again reverts with InvalidInitialization
    // [X] given initialize is called with zero admin
    //   [X] it reverts with ZeroAdmin error
    // [X] given initialize is called with zero pauser
    //   [X] it reverts with ZeroPauser error

    MockPortalV2 internal portal;
    address internal admin;
    address internal pauser;

    function setUp() public {
        portal = new MockPortalV2();
        admin = makeAddr("admin");
        pauser = makeAddr("pauser");
    }

    /* ========== Constructor Tests ========== */

    function test_constructor_zeroPortal_reverts() public {
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.ZeroPortal.selector));
        new OrderBook(address(0));
    }

    /* ========== Implementation Initialization Tests ========== */

    function test_implementationCannotBeInitialized() public {
        // Deploy implementation directly
        OrderBook implementation = new OrderBook(address(portal));

        // Attempt to initialize the implementation should fail because _disableInitializers() was called in constructor
        vm.expectRevert(abi.encodeWithSelector(Initializable.InvalidInitialization.selector));
        implementation.initialize(admin, pauser);
    }

    /* ========== Proxy Initialization Tests ========== */

    function test_proxyInitialization_adminHasDefaultAdminRole() public {
        // Deploy implementation
        address implementation = address(new OrderBook(address(portal)));

        // Deploy proxy with initialization
        OrderBook orderBook = OrderBook(
            address(
                new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, admin, pauser))
            )
        );

        // Verify admin has DEFAULT_ADMIN_ROLE
        bytes32 defaultAdminRole = orderBook.DEFAULT_ADMIN_ROLE();
        assertTrue(orderBook.hasRole(defaultAdminRole, admin));
    }

    function test_proxyInitialization_pauserHasPauserRole() public {
        // Deploy implementation
        address implementation = address(new OrderBook(address(portal)));

        // Deploy proxy with initialization
        OrderBook orderBook = OrderBook(
            address(
                new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, admin, pauser))
            )
        );

        // Verify pauser has PAUSER_ROLE
        bytes32 pauserRole = orderBook.PAUSER_ROLE();
        assertTrue(orderBook.hasRole(pauserRole, pauser));
    }

    function test_proxyInitialization_portalIsSetCorrectly() public {
        // Deploy implementation
        address implementation = address(new OrderBook(address(portal)));

        // Deploy proxy with initialization
        OrderBook orderBook = OrderBook(
            address(
                new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, admin, pauser))
            )
        );

        // Verify portal is set correctly
        assertEq(orderBook.portal(), address(portal));
    }

    function test_proxyInitialization_erc712DomainIsCorrectlyInitialized() public {
        // Deploy implementation
        address implementation = address(new OrderBook(address(portal)));

        // Deploy proxy with initialization
        OrderBook orderBook = OrderBook(
            address(
                new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, admin, pauser))
            )
        );

        // Verify ERC712 domain is correctly initialized
        (
            ,
            // bytes1 fields
            string memory name,
            string memory version,
            uint256 chainId,
            address verifyingContract,
            ,
            // bytes32 salt
            // uint256[] memory extensions
        ) = orderBook.eip712Domain();

        assertEq(name, "M0 OrderBook");
        assertEq(version, "1");
        assertEq(chainId, block.chainid);
        assertEq(verifyingContract, address(orderBook));
    }

    function test_proxyCannotBeReinitialized() public {
        // Deploy implementation
        address implementation = address(new OrderBook(address(portal)));

        // Deploy proxy with initialization
        OrderBook orderBook = OrderBook(
            address(
                new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, admin, pauser))
            )
        );

        // Attempt to re-initialize should fail
        vm.expectRevert(abi.encodeWithSelector(Initializable.InvalidInitialization.selector));
        orderBook.initialize(admin, pauser);
    }

    /* ========== Initialize Parameter Validation Tests ========== */

    function test_initialize_zeroAdmin_reverts() public {
        address implementation = address(new OrderBook(address(portal)));

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.ZeroAdmin.selector));
        new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, address(0), pauser));
    }

    function test_initialize_zeroPauser_reverts() public {
        address implementation = address(new OrderBook(address(portal)));

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.ZeroPauser.selector));
        new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, admin, address(0)));
    }
}
