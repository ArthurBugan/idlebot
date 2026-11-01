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

/// One stop for HUD bars (Spec 023 hotbar XP): returns
/// `(level, xp_into_level, xp_for_next_level, fill_fraction)`.
/// `fill_fraction` is 1.0 exactly at the threshold boundary (the caller
/// will always see the higher level in the same tuple, so the bar shows the
/// just-emptied next ring).
pub fn level_progress(total_xp: u64) -> (u32, u64, u64, f32) {
    let mut level = 1u32;
    let mut remaining = total_xp;
    let mut needed = xp_for_next_level(1);
    while remaining >= needed {
        remaining -= needed;
        level += 1;
        needed = xp_for_next_level(level);
    }
    let frac = remaining as f32 / needed as f32;
    (level, remaining, needed, frac.min(1.0))
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

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn threshold_exactness() {
        // 100 + 400 = 500 XP reaches exactly level 3.
        assert_eq!(calculate_level(0), 1);
        assert_eq!(calculate_level(99), 1);
        assert_eq!(calculate_level(100), 2);
        assert_eq!(calculate_level(499), 2);
        assert_eq!(calculate_level(500), 3);
        assert_eq!(calculate_level(501), 3);
    }

    #[test]
    fn level_cap_at_100() {
        let huge = xp_for_next_level(99);
        let total = (1u32..=99).map(|l| xp_for_next_level(l)).sum::<u64>();
        assert_eq!(calculate_level(total - 1), 99);
        assert_eq!(calculate_level(total), 100);
        // No hard cap in the formula: very large XP keeps leveling.
        assert!(calculate_level(u64::MAX / 1000) > 100);
    }

    #[test]
    fn xp_remaining_boundaries() {
        assert_eq!(xp_remaining_for_next_level(1, 0), 100);
        assert_eq!(xp_remaining_for_next_level(1, 99), 1);
        assert_eq!(xp_remaining_for_next_level(1, 100), 0);
        // Level 3 needs 900 XP; with 600 total (100 past the 500 threshold),
        // 800 remain.
        assert_eq!(xp_remaining_for_next_level(3, 600), 800);
    }

    #[test]
    fn level_progress_tracks_the_ring_fill() {
        assert_eq!(level_progress(0), (1, 0, 100, 0.0));
        assert_eq!(level_progress(99), (1, 99, 100, 0.99));
        // 300 total = 200 into the L2 ring of 400 → half full.
        let (level, used, needed, frac) = level_progress(300);
        assert_eq!((level, used, needed), (2, 200, 400));
        assert!((frac - 0.5).abs() < 1e-6);
        // Exactly at the L3 threshold: level 3, fresh ring of 900.
        assert_eq!(level_progress(500), (3, 0, 900, 0.0));
        // Over the threshold: level already bumped, remainder fills the ring.
        let (level, used, needed, _) = level_progress(560);
        assert_eq!((level, used, needed), (3, 60, 900));
    }
}
