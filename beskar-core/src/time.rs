use core::num::NonZeroU64;
pub use core::time::Duration;

/// The amount of microseconds in a millisecond.
pub const MICROS_PER_MILLI: u64 = 1_000;
/// The amount of milliseconds in a second.
pub const MILLIS_PER_SEC: u64 = 1_000;
/// The amount of microseconds in a second.
pub const MICROS_PER_SEC: u64 = MILLIS_PER_SEC * MICROS_PER_MILLI;
/// The amount of nanoseconds in a microsecond.
pub const NANOS_PER_MICRO: u64 = 1_000;
/// The amount of nanoseconds in a millisecond.
pub const NANOS_PER_MILLI: u64 = NANOS_PER_MICRO * MICROS_PER_MILLI;
/// The amount of nanoseconds in a second.
pub const NANOS_PER_SEC: u64 = NANOS_PER_MILLI * MILLIS_PER_SEC;

/// The amount of ticks per second of the timer.
///
/// The current unit is the microsecond.
pub const TPS: u64 = MICROS_PER_SEC;
/// The amount of ticks per millisecond of the timer.
///
/// The current unit is the microsecond.
pub const TICKS_PER_MILLI: u64 = MICROS_PER_MILLI;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A representation of an absolute time value, relative to an arbitrary start.
pub struct Instant {
    raw: u64,
}

impl Instant {
    pub const ORIGIN: Self = Self { raw: 0 };

    #[must_use]
    #[inline]
    /// Creates a new `Instant` from a raw representation.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the provided `raw` value is a valid representation of an `Instant`.
    pub const unsafe fn from_raw(raw: u64) -> Self {
        Self { raw }
    }

    #[must_use]
    #[inline]
    /// Returns the raw representation of this `Instant`.
    pub const fn to_raw(self) -> u64 {
        self.raw
    }

    #[must_use]
    #[inline]
    /// Computes `value * num / denom` without overflow, as long as both
    /// `num * denom` and the overall result fit.
    const fn mul_div_u64(value: u64, num: u64, denom: u64) -> u64 {
        let q = value / denom;
        let r = value % denom;
        q * num + r * num / denom
    }

    #[must_use]
    #[inline]
    pub fn checked_duration_since(self, earlier: Self, freq: u64) -> Option<Duration> {
        let raw_diff = self.raw.checked_sub(earlier.raw)?;
        let raw = Self::mul_div_u64(raw_diff, TPS, freq);
        let duration = Duration::from_micros(raw);
        Some(duration)
    }

    #[must_use]
    #[inline]
    pub fn checked_add_duration(self, duration: Duration, freq: u64) -> Option<Self> {
        let micros = u64::try_from(duration.as_micros()).ok()?;
        let raw = Self::mul_div_u64(micros, freq, TPS);
        let new_raw = self.raw.checked_add(raw)?;
        Some(Self { raw: new_raw })
    }
}

#[derive(Debug, Clone, Copy)]
/// Information about the high precision timer.
pub struct TimerInfo {
    /// The number of ticks per second of the timer.
    ///
    /// Set to `None` if no high-precision timer is available.
    pub ticks_per_ms: Option<NonZeroU64>,
    /// Whether the timer can be used directly from userspace.
    pub fastpath: bool,
}
