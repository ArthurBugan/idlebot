# Tasks 011: Marketplace System

> **Implementation Checklist**

## Phase 1: Publish Template
- [✓] **T1.1** Publish template listing with title, description, github_url
- [ ] **T1.2** Create MarketListing struct (listing_id, seller, title, description, github_url, price_usdt, published_at, sold)
- [ ] **T1.3** Validate listing has required fields
- [ ] **T1.4** Deduct USDT from seller

## Phase 2: Purchase Template
- [✓] **T1.5** Purchase template listing
- [ ] **T1.6** Transfer USDT to seller
- [ ] **T1.7** Mark listing as sold
- [ ] **T1.8** Notify seller of purchase

## Phase 3: Unsold Listings
- [ ] **T1.9** List unsold listings in "All Templates" view
- [ ] **T1.10** Show listing price, seller, and upload date
- [ ] **T1.11** Clean up listings older than 1 hour

## Phase 4: UI Display
- [ ] **T1.12** Render marketplace grid with listing cards
- [ ] **T1.13** Show "template" badge on listings
- [ ] **T1.14** Display unsold count in header

## Phase 5: Withdraw Unsold
- [ ] **T1.15** Implement withdraw_funds function for unsold listings
- [ ] **T1.16** Transfer unsold USDT to seller
- [ ] **T1.17** Mark listing as withdrawn

## Phase 6: Testing
- [✓] **T1.18** Test publish listing
- [✓] **T1.19** Test purchase listing
- [✓] **T1.20** Test withdraw unsold listing
- [ ] **T1.21** Test USDT transfer
- [ ] **T1.22** Test cooldown prevents spam
