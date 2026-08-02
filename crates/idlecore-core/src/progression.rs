//! Level progression system
//!
//! Formula: xp_for_next_level(level) = 100 * level^2
//! Returns the level a player reaches for a given total XP.

/// XP required to reach the next level
/// Formula: 100 * level^2 (e.g., level 1 needs 100 XP, level 2 needs 400 XP, etc.)
pub fn xp_for_next_level(level: u32) -> u64 {
    100 * (level as u64).pow(2)
}

/// Calculate the player's level from their total accumulated XP.
/// Returns the current level (1-indexed).
pub fn calculate_level(total_xp: u64) -> u32 {
    let mut level = 1u32;
    let mut xp_needed = 100u64;
    let mut remaining = total_xp;
    while remaining >= xp_needed {
        remaining -= xp_needed;
        level += 1;
        xp_needed = xp_for_next_level(level);
    }
    level
}

/// Get the XP remaining before the next level-up
pub fn xp_remaining_for_next_level(level: u32, total_xp: u64) -> u64 {
    // Calculate XP needed for current level
    let xp_needed = xp_for_next_level(level);
    // Calculate XP required to reach this level from level 1
    let xp_to_reach_level: u64 = (1..level).map(|l| xp_for_next_level(l)).sum();
    // XP used at current level
    let xp_used_at_level = total_xp.saturating_sub(xp_to_reach_level);
    // Remaining XP needed
    xp_needed.saturating_sub(xp_used_at_level)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_xp_for_next_level() {
        assert_eq!(super::xp_for_next_level(1), 100);
        assert_eq!(super::xp_for_next_level(2), 400);
        assert_eq!(super::xp_for_next_level(3), 900);
        assert_eq!(super::xp_for_next_level(10), 10_000);
    }

    #[test]
    fn test_calculate_level() {
        assert_eq!(super::calculate_level(0), 1);
        assert_eq!(super::calculate_level(99), 1);
        assert_eq!(super::calculate_level(100), 2);
        assert_eq!(super::calculate_level(499), 2);
        assert_eq!(super::calculate_level(500), 3);
    }

    #[test]
    fn test_xp_remaining() {
        // Level 1, 0 XP: need 100 for level 1→2
        assert_eq!(super::xp_remaining_for_next_level(1, 0), 100);
        // Level 1, 100 XP: have enough, level up to 2
        assert_eq!(super::xp_remaining_for_next_level(1, 100), 0);
        // Level 1, 150 XP: exceeded, still 0
        assert_eq!(super::xp_remaining_for_next_level(1, 150), 0);
        // Level 2, 300 XP: used 200 at level 2 (300-100), need 400, so 200 remaining
        assert_eq!(super::xp_remaining_for_next_level(2, 300), 200);
        // Level 2, 500 XP: used 400 at level 2 (500-100), need 400, so 0 remaining
        assert_eq!(super::xp_remaining_for_next_level(2, 500), 0);
    }
}
