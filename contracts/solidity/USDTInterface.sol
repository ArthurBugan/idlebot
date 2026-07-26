// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title USDTInterface
 * @notice ERC-20 interface for USDT on Polygon
 * @dev USDT on Polygon: 0xc2132D05D31c914a87C6611C10748AEb04B58e8F
 */
interface USDTInterface {
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 amount) external returns (bool);
    function allowance(address owner, address spender) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function decimals() external view returns (uint8);
}
