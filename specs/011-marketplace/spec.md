# Spec 011: Marketplace System

> **Objective:** Implement marketplace for publishing, browsing, and purchasing AI agents and code templates

## Problem Statement

Players need a way to trade AI agents and code templates using USDT. The marketplace should support publishing listings, browsing available items, and completing purchases.

## Proposed Solution

- Publish: Create listing with title, description, GitHub URL, USDT price (costs 50G)
- Browse: View all public listings
- Purchase: Pay USDT via smart contract
- Delivery: Listing marked as sold, buyer gets GitHub access

## Requirements

### Functional Requirements
1. FR1: Publish template listing
2. FR2: Set price in USDT
3. FR3: Browse all public listings
4. Figure out pricing tier
5. FR5: Purchase listing with USDT
6. FR6: Mark listing as sold after purchase
7. FR7: Seller receives USDT (minus platform fee)
8. FR8: Listings expire after 30 days

### Non-Functional Requirements
1. NFR1: Server-authoritative marketplace
2. NFR2: Smart contract handles USDT transfers
3. NFR3: Listing search/filter by category
4. NFR4: Platform fee: 5% of sale price

## Design

### Marketplace Data Model
```rust
struct MarketplaceListing {
    id: Uuid,
    seller_id: UUID,
    title: String,
    description: String,
    github_url: String,
    price_usdt: u64,
    category: ListingCategory,
    published_at: Instant,
    expires_at: Instant,
    is_sold: bool,
    buyer_id: Option<UUID>,
}

enum ListingCategory {
    Agent,
    Code,
    Template,
    Snippet,
}

impl MarketplaceListing {
    fn is_expired(&self) -> bool {
        self.published_at.elapsed() > Duration::from_secs(30 * 24 * 3600)
    }
}
```

### Marketplace System
```rust
struct MarketplaceSystem {
    listings: Vec<MarketplaceListing>,
    platform_fee_percent: f64, // 5%
}

impl MarketplaceSystem {
    fn publish_listing(&mut self, seller: &mut Player, listing: NewListing) -> Result<MarketplaceListing> {
        if seller.gold < 50 {
            return Err(MarketplaceError::InsufficientGold);
        }
        
        seller.gold -= 50;
        
        let listing = MarketplaceListing {
            id: Uuid::new_v4(),
            seller_id: seller.id,
            title: listing.title,
            description: listing.description,
            github_url: listing.github_url,
            price_usdt: listing.price_usdt,
            category: listing.category,
            published_at: Instant::now(),
            expires_at: Instant::now() + Duration::from_secs(30 * 24 * 3600),
            is_sold: false,
            buyer_id: None,
        };
        
        self.listings.push(listing);
        Ok(listing)
    }
    
    fn buy_listing(&mut self, buyer: &mut Player, listing_id: Uuid) -> Result<()> {
        let listing = self.get_listing_mut(listing_id)?;
        
        if listing.is_sold {
            return Err(MarketplaceError::AlreadySold);
        }
        
        if buyer.usdt < listing.price_usdt {
            return Err(MarketplaceError::InsufficientUSDT);
        }
        
        // Transfer USDT to seller (minus platform fee)
        let platform_fee = listing.price_usdt as f64 * self.platform_fee_percent / 100.0;
        let seller_receives = listing.price_usdt as f64 - platform_fee;
        
        buyer.usdt -= listing.price_usdt;
        self.update_seller_balance(listing.seller_id, seller_receives as u64);
        
        listing.is_sold = true;
        listing.buyer_id = Some(buyer.id);
        
        Ok(())
    }
}
```

### Smart Contract Integration
```solidity
contract TemplateMarket {
    struct Listing {
        address seller;
        string githubUrl;
        uint256 priceUSDT;
        bool isSold;
        address buyer;
    }
    
    mapping(bytes32 => Listing) public listings;
    
    function buyListing(bytes32 listingId) external payable {
        require(!listings[listingId].isSold, "Already sold");
        // Transfer USDT, mark as sold
    }
}
```

## Acceptance Criteria
- [ ] Can publish listing with 50G cost
- [ ] Listing appears in public browse
- [ ] Can purchase listing with USDT
- [ ] Platform fee deducted correctly
- [ ] Listing marked as sold after purchase
- [ ] Listings expire after 30 days
- [ ] Search/filter by category works

## Risks
- R1: Malicious GitHub URLs
- R2: USDT price fluctuation (stablecoin)
- R3: Dispute resolution for listings

## Open Questions
- Q1: Should there be listing reviews/ratings?
- Q2: Platform fee adjustable per deployment?
- Q3: Featured listings (premium)?
