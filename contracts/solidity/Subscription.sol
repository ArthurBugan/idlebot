// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./USDTInterface.sol";

/**
 * @title Subscription
 * @notice Premium subscription for increased template inventory
 * @dev Free tier: 50 templates. Premium (1 USDT/month): 500 templates
 */
contract Subscription {
    USDTInterface public usdt;

    uint256 public constant PREMIUM_PRICE = 1_000_000; // 1 USDT (6 decimals)
    uint256 public constant MONTH_SECONDS = 30 days;

    uint256 public constant FREE_LIMIT = 50;
    uint256 public constant PREMIUM_LIMIT = 500;

    mapping(address => uint256) public expiration;

    event Subscribed(address indexed user, uint256 expiresAt);
    event Unsubscribed(address indexed user);

    constructor(address _usdt) {
        usdt = USDTInterface(_usdt);
    }

    function subscribe() external {
        uint256 currentExpiry = expiration[msg.sender];
        uint256 newExpiry;

        if (currentExpiry > block.timestamp) {
            newExpiry = currentExpiry + MONTH_SECONDS;
        } else {
            newExpiry = block.timestamp + MONTH_SECONDS;
        }

        require(
            usdt.transferFrom(msg.sender, address(this), PREMIUM_PRICE),
            "USDT transfer failed"
        );

        expiration[msg.sender] = newExpiry;
        emit Subscribed(msg.sender, newExpiry);
    }

    function isActive(address user) external view returns (bool) {
        return expiration[user] > block.timestamp;
    }

    function getLimit(address user) external view returns (uint256) {
        if (isActive(user)) {
            return PREMIUM_LIMIT;
        }
        return FREE_LIMIT;
    }
}
