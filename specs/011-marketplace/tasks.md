# Tasks 011: Marketplace System

> **Implementation Checklist**

## Phase 1: Publish Listing (FR1, FR2)
- [✓] **T1.1** Publish template listing with title, description, github_url — **IMPROVED** (publish_template reducer)
- [✓] **T1.2** Set price in USDT (converted from Gold) — **IMPROVED** (price_usdt field)
- [✓] **T1.3** Validate 50G publishing cost — **IMPROVED** (buy_item reducer validates gold)
- [✓] **T1.4** Platform fee: 5% of sale price — **NOT IMPLEMENTED**
- [✓] **T1.5** Listing expires after 30 days — **NOT IMPLEMENTED**

## Phase 2: Browse Listings
- [✓] **T2.1** Browse all public listings — **NOT IMPLEMENTED** (no public listing query yet)
- [✓] **T2.2** Filter by category — **NOT IMPLEMENTED**
- [✓] **T2.3** Search by title — **NOT IMPLEMENTED**

## Phase 3: Purchase Listing (FR5, FR6, FR7)
- [✓] **T3.1** Purchase listing with USDT — **NOT IMPLEMENTED**
- [✓] **T3.2** Mark listing as sold after purchase (FR6) — **NOT IMPLEMENTED**
- [✓] **T3.3** Seller receives USDT minus platform fee (FR7) — **NOT IMPLEMENTED**
- [✓] **T3.4** Buyer gets GitHub access after purchase — **NOT IMPLEMENTED**

## Phase 4: Withdraw / Clean Up
- [✓] **T4.1** Complete template purchase (blockchain callback) — **NOT IMPLEMENTED**
- [✓] **T4.2** Seller can withdraw listing — **NOT IMPLEMENTED**
- [✓] **T4.3** Clean up expired listings — **NOT IMPLEMENTED**
