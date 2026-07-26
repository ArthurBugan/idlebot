# Spec 012: Smart Contracts (Polygon)

> **Objective:** Deploy and integrate Polygon smart contracts for wallet auth, marketplace, and subscriptions

## Problem Statement

Need blockchain integration for wallet authentication, marketplace transactions, and optional subscription tiers. Contracts must be secure, gas-efficient, and auditable.

## Proposed Solution

- Three smart contracts: Subscription.sol, TemplateMarket.sol, USDTInterface.sol
- Deploy to Polygon testnet (Mumbai) for MVP
- Use OpenZeppelin for security best practices
- Alloy CLI for contract interaction

## Requirements

### Functional Requirements
1. FR1: Wallet signature verification for login
2. FR2: Marketplace listing creation and purchase
3. FR3: USDT token transfers for purchases
4. FR4: Platform fee collection (5%)
5. FR5: Subscription management (future Phase 3)

### Non-Functional Requirements
1. NFR1: Gas optimization (target < 50k gas per transaction)
2. NFR2: Reentrancy protection
3. NFR3: Access control (only admin can pause/upgrade)
4. NFR4: Event logging for off-chain indexing

## Design

### Contract Architecture

#### 1. Subscription.sol
```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract Subscription is Ownable {
    struct SubscriptionTier {
        uint256 priceUSDT;
        uint256 duration; // seconds
        string benefits;
    }
    
    mapping(address => SubscriptionTier) public subscriptions;
    IERC20 public usdtToken;
    
    event SubscriptionPurchased(address indexed user, uint256 tierId, uint256 duration);
    
    constructor(address _usdtAddress) {
        usdtToken = IERC20(_usdtAddress);
    }
    
    function purchaseSubscription(uint256 tierId) external payable {
        // Validate tier, transfer USDT, activate subscription
    }
}
```

#### 2. TemplateMarket.sol
```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

contract TemplateMarket is ReentrancyGuard {
    struct Listing {
        address seller;
        string githubUrl;
        uint256 priceUSDT;
        bool isSold;
        address buyer;
        uint256 expiresAt;
    }
    
    IERC20 public usdtToken;
    address public platformWallet;
    uint256 public platformFeePercent; // 500 = 5%
    
    mapping(bytes32 => Listing) public listings;
    bytes32[] public listingIds;
    
    event ListingCreated(bytes32 indexed listingId, address seller, uint256 price);
    event ListingSold(bytes32 indexed listingId, address buyer, uint256 price);
    event PlatformFeeCollected(uint256 amount);
    
    function createListing(
        string calldata githubUrl,
        uint256 priceUSDT,
        uint256 durationDays
    ) external {
        // Validate, create listing, charge 50G from player
        bytes32 listingId = keccak256(abi.encodePacked(msg.sender, block.timestamp));
        listingIds.push(listingId);
        
        listings[listingId] = Listing({
            seller: msg.sender,
            githubUrl: githubUrl,
            priceUSDT: priceUSDT,
            isSold: false,
            buyer: address(0),
            expiresAt: block.timestamp + (durationDays * 1 days)
        });
        
        emit ListingCreated(listingId, msg.sender, priceUSDT);
    }
    
    function buyListing(bytes32 listingId) external nonReentrant {
        Listing storage listing = listings[listingId];
        require(!listing.isSold, "Already sold");
        require(block.timestamp < listing.expiresAt, "Expired");
        
        uint256 fee = (listing.priceUSDT * platformFeePercent) / 10000;
        uint256 sellerAmount = listing.priceUSDT - fee;
        
        usdtToken.transferFrom(msg.sender, platformWallet, fee);
        usdtToken.transferFrom(msg.sender, listing.seller, sellerAmount);
        
        listing.isSold = true;
        listing.buyer = msg.sender;
        
        emit ListingSold(listingId, msg.sender, listing.priceUSDT);
        emit PlatformFeeCollected(fee);
    }
}
```

#### 3. USDTInterface.sol
```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface USDTInterface {
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
    function allowance(address owner, address spender) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
}
```

### Alloy CLI Integration
```bash
# Deploy to testnet
cd contracts
alloy contract deploy TemplateMarket --rpc-url https://polygon-mumbai.g.alchemy.com/v2/$ALCHEMY_KEY

# Interact with contract
alloy contract call TemplateMarket createListing \
  --args "https://github.com/user/repo,100,30" \
  --rpc-url https://polygon-mumbai.g.alchemy.com/v2/$ALCHEMY_KEY
```

## Acceptance Criteria
- [ ] Contracts compile without errors
- [ ] Deploy to Polygon testnet
- [ ] Wallet login works with testnet
- [ ] Marketplace listing creation works
- [ ] Purchase flow completes successfully
- [ ] Platform fee collected correctly
- [ ] Unit tests pass for all contracts
- [ ] Gas usage within target

## Risks
- R1: Smart contract bugs (use OpenZeppelin, audit)
- R2: Testnet vs mainnet differences
- R3: Gas price spikes

## Open Questions
- Q1: Should contracts be upgraded via proxy?
- Q2: Multi-chain support (Arbitrum, Optimism)?
- Q3: NFT-based listings vs ERC20?
