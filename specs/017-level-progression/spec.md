# Spec 017: Level Progression System

> **Objective:** To formally define, implement, and verify the system responsible for calculating and tracking player progression through defined levels based on total experience points ($\text{XP}$) earned.

## Problem Statement
The IdleBot project requires a robust, server-authoritative system to translate accumulated experience points ($\text{XP}$) into a discrete "Level" visible to the player. This progression must be smooth, technically sound, and provide clear feedback to the player regarding their current standing and next goal.

## Proposed Solution
Progression is based on an exponentially scaling requirement per level, tying into the overall player experience accumulation model. All level calculations must reside on the server to prevent client-side exploitation.

## Requirements
### Functional Requirements
1. **FR1: Level Calculation:** Calculate the current player level given total accrued XP using the defined progression formula.
2. **FR2: Level Display:** The current player level must be visible on the player avatar/UI for all players.
3. **FR3: XP Progress Display:** The system must provide real-time display indicating XP needed to reach the next level (e.g., "75 / 100 XP to Level 2").
4. **FR4: Level-Up Event:** A server-side notification must be triggered upon level advancement, notifying all relevant clients.
5. **FR5: Persistence:** Both the current level and the total accrued XP must be persisted in the database on every significant progression milestone.

### Non-Functional Requirements
1. **NFR1: Server Authority:** All level progression logic (XP accrual, level advancement) must be server-side authoritative.
2. **NFR2: O(1) Cache Performance:** For frequent reads (e.g., UI rendering), the current level must be cached for near O(1) lookup time.

## Design
### Data Model (Rust Structs)
The player state requires tracking both the total earned XP and the level for accurate progression tracking.

```rust
pub struct ProgressionState {
    pub current_level: u32,
    pub total_xp: u64,
    // Other fields like: level_up_timestamp, etc.
}
```

### Algorithm (Rust Implementation)
The progression relies on two core functions matching the PROPOSAL Appendix 9.2 logic:

**Progression Formula:**
The experience required to reach the *next* level, starting from `current_level`, is defined as:
$$\text{XP}_{\text{next\_level}}(\text{level}) = 100 \cdot (\text{level})^2$$

**Rust Implementation:**

```rust
pub fn xp_for_next_level(level: u32) -> u64 {
    // Calculates the threshold needed to advance *beyond* the current level.
    100 * (level as u64).pow(2)
}

pub fn calculate_level(total_xp: u64) -> u32 {
    let mut level = 1u32;
    // Starting requirement for Level 1 -> Level 2 is 100 XP
    let mut xp_needed = 100u64; 
    let mut remaining = total_xp;
    
    // Loop condition checks if total_xp meets the requirement for the *next* level.
    while remaining >= xp_needed {
        remaining -= xp_needed;
        level += 1;
        // Calculate the requirement for the *new* next level (i.e., the level after the current one)
        xp_needed = Self::xp_for_next_level(level);
    }
    level
}
```

### XP Contribution Mapping
XP is accrued from various activities:

| Activity | XP Contribution |
| :--- | :--- |
| Plant | +5 |
| Harvest | +10 |
| Clean | +15 |
| Idle Gains | See Detailed Table |

**Idle Gains Detailed Table (Example: Levels 1-10):**

| Level Range | XP to Achieve Next Level | XP Earned in Range |
| :--- | :--- | :--- |
| 1 $\to$ 2 | 100 | $\approx 100$ |
| 2 $\to$ 3 | 400 | $\approx 300$ |
| 3 $\to$ 4 | 900 | $\approx 1300$ |
| 4 $\to$ 5 | 1600 | $\approx 2500$ |
| $\dots$ | $\dots$ | $\dots$ |

*(Note: This table must be fully populated during final integration.)*

## Acceptance Criteria
- [X] [FR1] Level is correctly calculated from total XP through the progression function.
- [X] [FR2] Current level is rendered on the player avatar across all clients.
- [X] [FR3] XP bar accurately reflects `(Total_XP - XP_at_Previous_Level) / XP_Needed_For_Next_Level`.
- [X] [FR4] Server successfully broadcasts a Level Up event upon transition.
- [X] [FR5] Database persists `total_xp` and `current_level` atomically upon progression.

## Risks
- R1: Client manipulation attempt to inflate XP before server sync. (Mitigated by NFR1)
- R2: Initial calculation failure causing users to incorrectly believe they earned XP. (Mitigation: Extensive integration testing against Spec 001).

## Open Questions
- Q1: How does the Idle Gain calculation integrate with the XP required by the progression system? (Assuming Idle Gains contribute to `total_xp`).
- Q2: Are there any mid-game rewards tied to reaching specific levels? (Cosmetic/UI only for now).
