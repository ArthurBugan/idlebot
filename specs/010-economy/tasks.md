# Tasks 010: Economy System

> **Implementation Checklist**

## Phase 1: Currency Display
- [ ] **T1.1** Create EconomyPanel component showing all 3 currencies
- [ ] **T1.2** Display Gold balance
- [ ] **T1.3** Display XP balance
- [ ] **T1.4** Display Eco points balance

## Phase 2: Gold Income
- [ ] **T1.5** Calculate gold income from matches (50-150G per match)
- [ ] **T1.6** Calculate gold income from idle hours (1.5G/hour base)
- [ ] **T1.7** Calculate gold income from online time (1G/hour)

## Phase 3: Currency Exchange
- [ ] **T1.8** Implement convert gold to XP (2.5 XP per gold)
- [ ] **T1.9** Implement convert XP to gold (0.4 gold per XP)
- [ ] **T1.10** Implement convert USDT to gold (1 USDT = 200 gold)
- [ ] **T1.11** Implement convert gold to USDT (200 gold = 1 USDT)

## Phase 4: Cooldown System
- [ ] **T1.12** Implement 6-hour cooldown on currency conversion
- [ ] **T1.13** Display cooldown timer in UI
- [ ] **T1.14** Prevent conversion during cooldown

## Phase 5: USDT Withdrawal
- [ ] **T1.15** Implement withdraw gold to wallet (1 gold = 1 USDT)
- [ ] **T1.16** Calculate USDT amount from gold balance
- [ ] **T1.17** Implement 24-hour withdrawal cooldown
- [ ] **T1.18** Display pending transactions

## Phase 6: Testing
- [ ] **T1.19** Test currency display updates
- [ ] **T1.20** Test currency conversion rates
- [ ] **T1.21** Test cooldown prevents spam
- [ ] **T1.22** Test withdrawal process
