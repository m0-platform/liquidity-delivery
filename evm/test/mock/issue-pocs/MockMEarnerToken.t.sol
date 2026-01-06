// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

/// @notice Mock M Earner token simulating index-based yield accrual
contract MockMEarnerToken {
    event Transfer(address indexed from, address indexed to, uint256 amount);
    event Approval(address indexed owner, address indexed spender, uint256 amount);
    event IndexUpdated(uint128 oldIndex, uint128 newIndex);
    event FeeRateSet(address indexed account, uint256 feeRate);
    event YieldClaimed(address indexed account, address indexed feeRecipient, uint256 yield, uint256 fee);

    string public name;
    string public symbol;
    uint8 public immutable decimals;

    uint128 public currentIndex = 1e12;
    mapping(address => uint256) internal _principal;
    mapping(address => uint128) internal _depositIndex;
    mapping(address => uint256) public feeRate; // basis points (10000 = 100%)
    address public feeRecipient;

    mapping(address => mapping(address => uint256)) public allowance;

    address public owner;

    constructor(string memory name_, string memory symbol_, uint8 decimals_) {
        name = name_;
        symbol = symbol_;
        decimals = decimals_;
        owner = msg.sender;
        feeRecipient = msg.sender;
    }

    function setFeeRate(address account, uint256 feeRate_) external {
        require(msg.sender == owner, "not owner");
        require(feeRate_ <= 10000, "fee rate too high");
        feeRate[account] = feeRate_;
        emit FeeRateSet(account, feeRate_);
    }

    function setFeeRecipient(address feeRecipient_) external {
        require(msg.sender == owner, "not owner");
        feeRecipient = feeRecipient_;
    }

    // balance = principal * currentIndex / depositIndex (grows as index increases)
    function balanceOf(address account) public view returns (uint256) {
        if (_depositIndex[account] == 0) return 0;
        return (_principal[account] * currentIndex) / _depositIndex[account];
    }

    function totalSupply() public view returns (uint256) {
        return 0;
    }

    function accrueYield(uint256 basisPoints) external {
        require(msg.sender == owner, "not owner");
        uint128 oldIndex = currentIndex;
        currentIndex = uint128((uint256(currentIndex) * (10000 + basisPoints)) / 10000);
        emit IndexUpdated(oldIndex, currentIndex);
    }

    // Mitigation: with feeRate=100%, all yield routes to feeRecipient instead of getting stuck
    function claimExcessYield(address account) external {
        uint256 balance = balanceOf(account);
        uint256 originalPrincipal = _principal[account];
        uint256 yieldAmount = balance > originalPrincipal ? balance - originalPrincipal : 0;
        if (yieldAmount == 0) return;

        uint256 feeAmount = (yieldAmount * feeRate[account]) / 10000;
        uint256 newBalance = balance - feeAmount;
        _depositIndex[account] = currentIndex;
        _principal[account] = newBalance;

        if (feeAmount > 0 && feeRecipient != address(0)) {
            if (_depositIndex[feeRecipient] == 0) {
                _depositIndex[feeRecipient] = currentIndex;
            }
            _principal[feeRecipient] += feeAmount;
            emit Transfer(account, feeRecipient, feeAmount);
        }

        emit YieldClaimed(account, feeRecipient, yieldAmount, feeAmount);
    }

    function accruedYield(address account) external view returns (uint256) {
        uint256 balance = balanceOf(account);
        uint256 originalPrincipal = _principal[account];
        return balance > originalPrincipal ? balance - originalPrincipal : 0;
    }

    function setIndex(uint128 newIndex) external {
        require(msg.sender == owner, "not owner");
        uint128 oldIndex = currentIndex;
        currentIndex = newIndex;
        emit IndexUpdated(oldIndex, newIndex);
    }

    function approve(address spender, uint256 amount) public virtual returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transfer(address to, uint256 amount) public virtual returns (bool) {
        return _transfer(msg.sender, to, amount);
    }

    function transferFrom(address from, address to, uint256 amount) public virtual returns (bool) {
        uint256 allowed = allowance[from][msg.sender];
        if (allowed != type(uint256).max) allowance[from][msg.sender] = allowed - amount;
        return _transfer(from, to, amount);
    }

    // Key: transfer deducts less principal than expected, leaving residual yield in sender
    function _transfer(address from, address to, uint256 amount) internal returns (bool) {
        uint256 fromBalance = balanceOf(from);
        require(fromBalance >= amount, "insufficient balance");

        uint256 principalToDeduct = (amount * _depositIndex[from]) / currentIndex;
        _principal[from] -= principalToDeduct;

        if (_depositIndex[to] == 0) {
            _depositIndex[to] = currentIndex;
        }

        uint256 principalToAdd = (amount * _depositIndex[to]) / currentIndex;
        _principal[to] += principalToAdd;

        emit Transfer(from, to, amount);
        return true;
    }

    function mint(address to, uint256 amount) external {
        if (_depositIndex[to] == 0) {
            _depositIndex[to] = currentIndex;
        }
        uint256 principalToAdd = (amount * _depositIndex[to]) / currentIndex;
        _principal[to] += principalToAdd;
        emit Transfer(address(0), to, amount);
    }

    function principalOf(address account) external view returns (uint256) {
        return _principal[account];
    }

    function depositIndexOf(address account) external view returns (uint128) {
        return _depositIndex[account];
    }
}
