// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

/// @notice Mock ERC20 token with configurable transfer fee
/// @dev Fee is in basis points (100 = 1%)
contract MockFeeToken {
    event Transfer(address indexed from, address indexed to, uint256 amount);
    event Approval(address indexed owner, address indexed spender, uint256 amount);

    string public name;
    string public symbol;
    uint8 public immutable decimals;
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    uint256 public feePercent; // in basis points (100 = 1%)
    address public owner;

    constructor(string memory name_, string memory symbol_, uint8 decimals_) {
        name = name_;
        symbol = symbol_;
        decimals = decimals_;
        owner = msg.sender;
        feePercent = 0;
    }

    function setFeePercent(uint256 feePercent_) external {
        require(msg.sender == owner, "not owner");
        feePercent = feePercent_;
    }

    function approve(address spender, uint256 amount) public virtual returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transfer(address to, uint256 amount) public virtual returns (bool) {
        uint256 fee = (amount * feePercent) / 10000;
        uint256 amountAfterFee = amount - fee;

        balanceOf[msg.sender] -= amount;

        unchecked {
            if (fee > 0) {
                balanceOf[address(0xdead)] += fee; // burn fee
            }
            balanceOf[to] += amountAfterFee;
        }

        if (fee > 0) {
            emit Transfer(msg.sender, address(0xdead), fee);
        }
        emit Transfer(msg.sender, to, amountAfterFee);

        return true;
    }

    function transferFrom(address from, address to, uint256 amount) public virtual returns (bool) {
        uint256 allowed = allowance[from][msg.sender];
        if (allowed != type(uint256).max) allowance[from][msg.sender] = allowed - amount;

        uint256 fee = (amount * feePercent) / 10000;
        uint256 amountAfterFee = amount - fee;

        balanceOf[from] -= amount;

        unchecked {
            if (fee > 0) {
                balanceOf[address(0xdead)] += fee; // burn fee
            }
            balanceOf[to] += amountAfterFee;
        }

        if (fee > 0) {
            emit Transfer(from, address(0xdead), fee);
        }
        emit Transfer(from, to, amountAfterFee);

        return true;
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function _mint(address to, uint256 amount) internal virtual {
        totalSupply += amount;

        unchecked {
            balanceOf[to] += amount;
        }

        emit Transfer(address(0), to, amount);
    }

    function burn(uint256 amount) external {
        _burn(msg.sender, amount);
    }

    function _burn(address from, uint256 amount) internal virtual {
        balanceOf[from] -= amount;

        unchecked {
            totalSupply -= amount;
        }

        emit Transfer(from, address(0), amount);
    }
}
