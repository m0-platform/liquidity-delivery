// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

/// @notice Mock ERC20 token that can rebase DOWN (decrease all balances)
/// @dev Simulates deflationary rebasing tokens like AMPL during contraction
contract MockRebaseDownToken {
    event Transfer(address indexed from, address indexed to, uint256 amount);
    event Approval(address indexed owner, address indexed spender, uint256 amount);
    event Rebase(uint256 oldTotalSupply, uint256 newTotalSupply);

    string public name;
    string public symbol;
    uint8 public immutable decimals;
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    address public owner;

    constructor(string memory name_, string memory symbol_, uint8 decimals_) {
        name = name_;
        symbol = symbol_;
        decimals = decimals_;
        owner = msg.sender;
    }

    /// @notice Rebase DOWN - decrease all balances by a percentage
    /// @param percentageDown Percentage to decrease (in basis points, 1000 = 10%)
    function rebaseDown(uint256 percentageDown) external {
        require(msg.sender == owner, "not owner");
        require(percentageDown <= 10000, "percentage too high");

        uint256 oldTotalSupply = totalSupply;
        uint256 multiplier = 10000 - percentageDown;

        // Note: In a real rebasing token, this would affect ALL balances proportionally
        // For simplicity, we're just tracking totalSupply change here
        // The actual balance decrease happens via the rebaseDownAccount function
        totalSupply = (totalSupply * multiplier) / 10000;

        emit Rebase(oldTotalSupply, totalSupply);
    }

    /// @notice Manually decrease a specific account's balance (simulating rebase effect)
    /// @param account The account to rebase
    /// @param percentageDown Percentage to decrease (in basis points, 1000 = 10%)
    function rebaseDownAccount(address account, uint256 percentageDown) external {
        require(msg.sender == owner, "not owner");
        require(percentageDown <= 10000, "percentage too high");

        uint256 oldBalance = balanceOf[account];
        uint256 multiplier = 10000 - percentageDown;
        uint256 newBalance = (oldBalance * multiplier) / 10000;

        balanceOf[account] = newBalance;
        totalSupply = totalSupply - oldBalance + newBalance;

        // Emit transfer to zero to represent the "burned" portion
        if (oldBalance > newBalance) {
            emit Transfer(account, address(0), oldBalance - newBalance);
        }
    }

    function approve(address spender, uint256 amount) public virtual returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transfer(address to, uint256 amount) public virtual returns (bool) {
        balanceOf[msg.sender] -= amount;

        unchecked {
            balanceOf[to] += amount;
        }

        emit Transfer(msg.sender, to, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) public virtual returns (bool) {
        uint256 allowed = allowance[from][msg.sender];
        if (allowed != type(uint256).max) allowance[from][msg.sender] = allowed - amount;

        balanceOf[from] -= amount;

        unchecked {
            balanceOf[to] += amount;
        }

        emit Transfer(from, to, amount);
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
