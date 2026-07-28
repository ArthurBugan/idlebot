# Tasks 014: Player Identity Management

> **Implementation Checklist**

## Phase 1: Identity Data
- [ ] **T1.1** Player profile stored in SpacetimeDB
- [ ] **T1.2** Avatar (unique per player)
- [ ] **T1.3** Display name (unique per player)
- [ ] **T1.4** Total XP earned (persistent)

## Phase 2: Identity Verification
- [ ] **T1.5** Verify wallet signature on connect
- [ ] **T1.6** Verify blockchain transaction signature
- [ ] **T1.7** Link wallet address to player record

## Phase 3: Profile Display
- [ ] **T1.8** Render player avatar on hex
- [ ] **T1.9** Render player display name above avatar
- [ ] **T1.10** Render player level badge

## Phase 4: Persistence
- [ ] **T1.11** Save avatar hash in DB
- [ ] **T1.12** Save display name in DB
- [ ] **T1.13** Restore identity on reconnect

## Phase 5: Privacy
- [ ] **T1.14** Show full identity only to hex mates
- [ ] **T1.15** Show minimal info to strangers

## Phase 6: Testing
- [✓] **T1.16** Test player spawn with identity
- [✓] **T1.17** Test identity display
- [✓] **T1.18** Test persist across sessions
