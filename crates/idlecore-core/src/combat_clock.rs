//! Spec 037 — THE TICKENING: millisecond combat on a fine scheduler
//! (user approval 2026-11-06, audit decision G.1: combat beats at 250 ms
//! quanta while the world keeps its 1 s march).
//!
//! All combat timing lives in unix MILLISECONDS (integer). The 1 s world
//! tick stays untouched (march, regen, spawners); strike beats, i-frames,
//! attack cadence and the move-lock commit at the fine grain.
//!
//! Constitution: integer-only math, server-authoritative, pure helpers
//! unit-tested. The server enforces every window; the client only predicts.

// ---------------------------------------------------------------------------
// Cadence constants (ms)
// ---------------------------------------------------------------------------

/// The combat scheduler's quanta: every strike beat, i-frame and lock
/// resolves on a multiple of this.
pub const COMBAT_TICK_MS: u64 = 250;

/// The dodge i-frame window: the WHOLE 0.6 s step, not one server second
/// (the client burst already runs 0.6 s — the server window now matches,
/// so a strike landing inside the step never clips its tail).
pub const DODGE_IFRAME_MS: u64 = 600;

/// Dodge cooldown between steps.
pub const DODGE_CD_MS: u64 = 900;

/// Attack move-lock: the swing owns the body for this long after the
/// reducer fires (the clip animation, client side, runs the same length —
/// commitment is real: you cannot walk out of your own strike).
pub fn weapon_commit_ms(anim: crate::player_combat::WeaponAnim) -> u64 {
    use crate::player_combat::WeaponAnim as W;
    match anim {
        W::Sword => 550,
        W::Bow => 700, // the draw is the commitment
        W::Axe => 800, // heavy two-handed
        W::Sickle => 450,
        W::Pickaxe => 650,
        W::Hoe => 500,
        W::Shovel => 500,
        W::Fists => 260,
    }
}

// ---------------------------------------------------------------------------
// Pure helpers (ms arithmetic, unit-tested)
// ---------------------------------------------------------------------------

/// `true` when the fine combat clock owes a resolve at `now_ms`.
/// Mirrors the 1 s tick's equality check but in ms.
#[inline]
pub fn due(now_ms: u64, target_ms: u64) -> bool {
    target_ms != 0 && now_ms >= target_ms
}

/// Phase of a telegraphed strike, all in ms.
/// Quiet: nothing armed or beyond the read window.
/// Wind: the last `WIND_MS` before the beat — the telegraph the client
/// renders.
/// Strike: the beat itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatPhase {
    Quiet,
    Wind,
    Strike,
}

/// The windup telegraph length: how long the client sees the mob pose
/// before the strike lands. Fresh per-strike, not a config column.
pub const WINDUP_MS: u64 = 900;

/// Phase of the telegraph at `now_ms` for a strike armed at `strike_ms`.
/// The wind is the window `[strike_ms - WINDUP_MS, strike_ms)`; the
/// strike is exactly `now_ms >= strike_ms`.
#[inline]
pub fn beat_phase(now_ms: u64, strike_ms: u64) -> BeatPhase {
    if strike_ms == 0 {
        return BeatPhase::Quiet;
    }
    if now_ms >= strike_ms {
        return BeatPhase::Strike;
    }
    if strike_ms - now_ms <= WINDUP_MS {
        return BeatPhase::Wind;
    }
    BeatPhase::Quiet
}

/// Arm a new strike schedule: `now_ms + period_ms` in ms, staggered by
/// mob id so a pack's beats don't land unison. Integer-clean (no float).
#[inline]
pub fn arm_strike(now_ms: u64, period_ms: u64, mob_id: u64) -> u64 {
    let period = period_ms.max(COMBAT_TICK_MS);
    now_ms + period + (mob_id % 4) * COMBAT_TICK_MS
}

/// Convert a 1-second-cadence legacy period into the fine clock.
/// The MobKind table carries `strike_period_secs` — the combat clock
/// buys 4× the resolution: period_ms = secs × 1000 / SPEEDUP, floored at
/// one tick so nothing degenerates to instant strikes.
pub const BEAT_SPEEDUP: u64 = 2; // 4 s slime period → 2 s of fine beats
#[inline]
pub fn period_ms(strike_period_secs: u32) -> u64 {
    let ms = u64::from(strike_period_secs) * 1000 / BEAT_SPEEDUP;
    ms.max(COMBAT_TICK_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quanta_of_250ms_is_the_floor() {
        assert_eq!(period_ms(4), 2000, "4 s slime period doubles speed to 2 s");
        assert_eq!(period_ms(1), 500, "1 s floor beat = 500 ms fine");
        assert_eq!(period_ms(0), COMBAT_TICK_MS, "floor at one tick");
    }

    #[test]
    fn beat_phase_reads_wind_then_strike() {
        let strike = 10_000u64;
        assert_eq!(beat_phase(9_000, strike), BeatPhase::Quiet, "2 beats out: quiet");
        assert_eq!(beat_phase(9_100, strike), BeatPhase::Wind, "exactly 900 in: wind");
        assert_eq!(beat_phase(9_999, strike), BeatPhase::Wind);
        assert_eq!(beat_phase(10_000, strike), BeatPhase::Strike, "the beat");
        assert_eq!(beat_phase(11_000, strike), BeatPhase::Strike, "stale stays strike");
        assert_eq!(beat_phase(5_000, 0), BeatPhase::Quiet, "unarmed quiet");
    }

    #[test]
    fn arm_staggers_the_pack() {
        let now = 10_000u64;
        let period = 2_000u64;
        // The stagger is mod-4 (4 quanta slots) — ids sharing a slot share
        // a beat BY DESIGN (a 6-mob pack cannot all stagger uniquely with
        // only 4 slots); the pin is the WINDOW, not slot uniqueness.
        for id in 0..8u64 {
            let b = arm_strike(now, period, id);
            assert!(b >= now + period, "never before the period");
            assert!(
                b <= now + period + 3 * COMBAT_TICK_MS,
                "never past 3 stagger slots"
            );
            // The stagger slot itself must be tick-aligned.
            assert_eq!((b - now) % COMBAT_TICK_MS, 0, "tick-aligned");
        }
        // At least two distinct beats across 4 ids (0..=3): the pack does
        // not land unison when the slots differ.
        let four: Vec<u64> = (0..4).map(|id| arm_strike(now, period, id)).collect();
        assert!(four.iter().any(|b| *b != four[0]), "ids 0..3 are staggered");
    }

    #[test]
    fn dodge_window_matches_the_client_burst() {
        assert_eq!(DODGE_IFRAME_MS, 600, "the client's 0.6 s step — server window mirrors it");
        assert_eq!(DODGE_CD_MS, 900, "dodge chained twice, never spammed");
        assert_eq!(COMBAT_TICK_MS, 250, "the fine clock");
    }

    #[test]
    fn weapon_commitment_is_real_but_tiny() {
        assert_eq!(weapon_commit_ms(crate::player_combat::WeaponAnim::Fists), 260);
        assert_eq!(weapon_commit_ms(crate::player_combat::WeaponAnim::Sword), 550);
        assert_eq!(weapon_commit_ms(crate::player_combat::WeaponAnim::Axe), 800);
        // Every weapon locks movement; none exceeds the axe.
        for anim in [
            crate::player_combat::WeaponAnim::Sword,
            crate::player_combat::WeaponAnim::Bow,
            crate::player_combat::WeaponAnim::Axe,
            crate::player_combat::WeaponAnim::Sickle,
            crate::player_combat::WeaponAnim::Pickaxe,
            crate::player_combat::WeaponAnim::Hoe,
            crate::player_combat::WeaponAnim::Shovel,
            crate::player_combat::WeaponAnim::Fists,
        ] {
            assert!(weapon_commit_ms(anim) >= 200, "commitment always real");
            assert!(weapon_commit_ms(anim) <= 800, "never a stun");
        }
    }

    #[test]
    fn due_is_truthy_only_when_armed() {
        assert!(!due(1_000, 0));
        assert!(!due(999, 1_000));
        assert!(due(1_000, 1_000));
        assert!(due(1_500, 1_000));
    }

    #[test]
    fn fine_beat_sim_30_traces_reproduce_the_fine_fsm() {
        // Spec 037 P6 — 30 procedural traces over the fine machine, exactly
        // as the 250 ms pass drives it: arm → wind (only in the last
        // WINDUP_MS) → strike → re-arm. The pins: every strike is preceded
        // by a wind on the MS clock, no strike ever lands before its
        // period, beats stay on the tick grid forever.
        for trace in 0..30u64 {
            let mob_id = trace * 11 + 5;
            // Per-kind-style period bands in seconds (1..=4) → ms conversion.
            let period_secs = 1 + (trace % 4) as u32;
            let period = period_ms(period_secs);
            let mut now: u64 = 50_000;
            let mut next = arm_strike(now, period, mob_id);
            let mut winds = 0u32;
            let mut strikes = 0u32;
            for _step in 0..30 {
                now += COMBAT_TICK_MS;
                match beat_phase(now, next) {
                    BeatPhase::Wind => {
                        winds += 1;
                        assert!(next > now, "wind is pre-beat");
                        assert!(
                            next - now <= WINDUP_MS,
                            "wind only airs within the telegraph window"
                        );
                    }
                    BeatPhase::Strike => {
                        strikes += 1;
                        assert_eq!(now, next, "strike lands exactly on the beat");
                        assert!(winds >= 1, " every strike had a wind");
                        next = arm_strike(now, period, mob_id);
                        winds = 0;
                    }
                    BeatPhase::Quiet => {}
                }
                // Grid: every step is tick-aligned.
                assert_eq!(now % COMBAT_TICK_MS, 0);
            }
            assert!(strikes > 0, "trace {trace} progressed to a strike");
            // A trailing wind is FINE (the window can outlive the trace —
            // the same rule as Spec 036's sim: the reverse, a strike with
            // no wind, is the unforgivable one; that pin is inline above).
        }
    }
}
