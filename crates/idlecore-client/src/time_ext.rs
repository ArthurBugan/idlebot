//! Wasm-compatible time helpers.
//!
//! `std::time::Instant`/`SystemTime` panic on `wasm32-unknown-unknown`
//! ("time not implemented on this platform"), so this module exposes a
//! drop-in `Instant` (backed by `performance.now()` on wasm) and a
//! `now_unix_secs()` (backed by `Date.now()` on wasm) that behave like the
//! std versions for the small surface the client actually uses.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::time::Duration;

    /// Milliseconds since the `performance` timeline started (monotonic).
    #[derive(Clone, Copy)]
    pub struct Instant(f64);

    impl Instant {
        pub fn now() -> Self {
            Instant(performance_now())
        }

        pub fn duration_since(&self, earlier: Instant) -> Duration {
            Duration::from_secs_f64((self.0 - earlier.0).max(0.0) / 1000.0)
        }

        pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
            self.duration_since(earlier)
        }

        pub fn elapsed(&self) -> Duration {
            Instant::now().duration_since(*self)
        }
    }

    impl std::ops::Sub<Instant> for Instant {
        type Output = Duration;
        fn sub(self, rhs: Instant) -> Duration {
            self.duration_since(rhs)
        }
    }

    impl std::ops::Add<Duration> for Instant {
        type Output = Instant;
        fn add(self, rhs: Duration) -> Instant {
            Instant(self.0 + rhs.as_secs_f64() * 1000.0)
        }
    }

    impl PartialEq for Instant {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Instant {}

    impl PartialOrd for Instant {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            self.0.partial_cmp(&other.0)
        }
    }
    impl Ord for Instant {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.0.partial_cmp(&other.0).unwrap_or(std::cmp::Ordering::Equal)
        }
    }

    fn performance_now() -> f64 {
        web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0)
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::Instant;

/// Current Unix time in seconds (matches `SystemTime`-based code on native).
pub fn now_unix_secs() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as u64
    }
}
