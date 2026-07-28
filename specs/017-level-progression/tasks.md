# Tasks 017: Level Progression System

> **Implementation Checklist**

## Phase 1: Progression Formula
- [✓] **T1.1** Calculate current level from total XP: `level = floor(log_{(level+1)^2 - level^2}(total_xp))`
- [✓] **T1.2** XP required for level N: `100 * N^2`
- [✓] **T1.3** XP gained from actions (plant +5, harvest +10, clean +15)
- [✓] **T1.4** XP gained from idle gains (server-calculated)

## Phase 2: Server Progression (NFR1)
- [ ] **T1.5** Level calculation runs on server only
- [✓] **T1.6** XP accrual is server-authoritative
- [✓] **T1.7** Database stores `total_xp` + `current_level` atomically
- [✓] **T1.8** Level up broadcast event to all connected clients

## Phase 3: Level Display (FR2)
- [✓] **T1.9** Render current level on player avatar
- [✓] **T1.10** XP progress bar visible: "X / Y XP to next level"

## Phase 4: XP Tracking (FR3)
- [✓] **T1.11** Track total XP earned (not per-action)
- [✓] **T1.12** XP bar updates in real-time as XP is earned
- [✓] **T1.13** Cache current level for O(1) lookup

## Phase 5: Persistence (FR5)
- [✓] **T1.14** Save total_xp on every XP gain
- [✓] **T1.15** Save current_level after level up
- [✓] **T1.16** Recover state on reconnect

## Phase 6: Testing
- [✓] **T2.1** Level 1 at 0 XP
- [✓] **T2.2** Level 2 at 100 XP
- [✓] **T2.3** Level 3 at 500 XP
- [✓] **T2.4** Level 4 at 1400 XP
- [✓] **T2.5** Level broadcast event fires at correct level
