use core::{
    fmt, ops,
    sync::atomic::{AtomicU64, Ordering},
};

/// The amount of microseconds in a millisecond.
pub const MICROS_PER_MILLI: u64 = 1_000;
/// The amount of microseconds in a second.
pub const MICROS_PER_SEC: u64 = 1_000_000;
/// The amount of milliseconds in a second.
pub const MILLIS_PER_SEC: u64 = 1_000;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A representation of an absolute time value, relative to an arbitrary start.
pub struct Instant {
    micros: u64,
}

impl Instant {
    /// The null instant, which is the starting point of time.
    pub const ZERO: Self = Self::from_micros(0);
    /// The maximum representable instant.
    pub const MAX: Self = Self::from_micros(u64::MAX);

    #[must_use]
    #[inline]
    /// Create a new `Instant` from a number of microseconds.
    pub const fn from_micros(micros: u64) -> Self {
        Self { micros }
    }
    #[must_use]
    #[inline]
    /// Create a new `Instant` from a number of milliseconds.
    pub const fn from_millis(millis: u64) -> Self {
        let micros = millis.saturating_mul(MICROS_PER_MILLI);
        Self::from_micros(micros)
    }
    #[must_use]
    #[inline]
    /// Create a new `Instant` from a number of seconds.
    pub const fn from_secs(secs: u64) -> Self {
        let micros = secs.saturating_mul(MICROS_PER_SEC);
        Self::from_micros(micros)
    }

    #[must_use]
    #[inline]
    /// The number of microseconds that have passed since the
    /// beginning of time, modulo the amount of seconds that
    /// have passed (refer to `Self::secs`).
    pub const fn micros(&self) -> u64 {
        self.micros % MICROS_PER_SEC
    }
    #[must_use]
    #[inline]
    /// The number of milliseconds that have passed since the
    /// beginning of time, modulo the amount of seconds that
    /// have passed (refer to `Self::secs`).
    pub const fn millis(&self) -> u64 {
        self.micros() / MICROS_PER_MILLI
    }

    #[must_use]
    #[inline]
    /// The number of whole seconds that have passed since the
    /// beginning of time.
    pub const fn secs(&self) -> u64 {
        self.micros / MICROS_PER_SEC
    }

    #[must_use]
    #[inline]
    /// The total number of milliseconds that have passed since
    /// the beginning of time.
    pub const fn total_micros(&self) -> u64 {
        self.micros
    }
    #[must_use]
    #[inline]
    /// The total number of milliseconds that have passed since
    /// the beginning of time.
    pub const fn total_millis(&self) -> u64 {
        self.total_micros() / MICROS_PER_MILLI
    }
}

impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{:0>3}s", self.secs(), self.millis())
    }
}

impl ops::Add<Duration> for Instant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self {
        let micros = self.micros.saturating_add(rhs.micros);
        Self { micros }
    }
}

impl ops::AddAssign<Duration> for Instant {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

impl ops::Sub<Duration> for Instant {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self {
        let micros = self.micros.saturating_sub(rhs.micros);
        Self { micros }
    }
}

impl ops::SubAssign<Duration> for Instant {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

impl ops::Sub<Self> for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Duration {
        let micros = self.micros.abs_diff(rhs.micros);
        Duration { micros }
    }
}

// Using `core::time::Duration` would force us to deal with `u128` frequently, which is not ideal.
/// A relative amount of time.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration {
    micros: u64,
}

impl Duration {
    /// The null duration.
    pub const ZERO: Self = Self::from_micros(0);
    /// The longest possible duration that can be represented.
    ///
    /// This is the maximum value of a `u64` as a microsecond duration.
    /// The approximate value is 5849 centuries.
    pub const MAX: Self = Self::from_micros(u64::MAX);

    #[must_use]
    #[inline]
    /// Create a new `Duration` from a number of microseconds.
    pub const fn from_micros(micros: u64) -> Self {
        Self { micros }
    }
    #[must_use]
    #[inline]
    /// Create a new `Duration` from a number of milliseconds.
    pub const fn from_millis(millis: u64) -> Self {
        let micros = millis.saturating_mul(MICROS_PER_MILLI);
        Self::from_micros(micros)
    }
    #[must_use]
    #[inline]
    /// Create a new `Duration` from a number of seconds.
    pub const fn from_secs(secs: u64) -> Self {
        let micros = secs.saturating_mul(MICROS_PER_SEC);
        Self::from_micros(micros)
    }

    #[must_use]
    #[inline]
    /// The number of microseconds that are represented,
    /// modulo the amount of seconds (refer to `Self::secs`).
    pub const fn micros(&self) -> u64 {
        self.micros % MICROS_PER_SEC
    }
    #[must_use]
    #[inline]
    /// The number of milliseconds that are represented,
    /// modulo the amount of seconds (refer to `Self::secs`).
    pub const fn millis(&self) -> u64 {
        self.micros() / MICROS_PER_MILLI
    }

    #[must_use]
    #[inline]
    /// The number of whole seconds in this `Duration`.
    pub const fn secs(&self) -> u64 {
        self.micros / MICROS_PER_SEC
    }

    #[must_use]
    #[inline]
    /// The total number of microseconds in this `Duration`.
    pub const fn total_micros(&self) -> u64 {
        self.micros
    }
    #[must_use]
    #[inline]
    /// The total number of milliseconds in this `Duration`.
    pub const fn total_millis(&self) -> u64 {
        self.total_micros() / MICROS_PER_MILLI
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{:03}s", self.secs(), self.millis())
    }
}

impl ops::Add<Self> for Duration {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let micros = self.micros.saturating_add(rhs.micros);
        Self { micros }
    }
}

impl ops::AddAssign<Self> for Duration {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl ops::Sub<Self> for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        let micros = self.micros.saturating_sub(rhs.micros);
        Self { micros }
    }
}

impl ops::SubAssign<Self> for Duration {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl<T: Into<u64>> ops::Mul<T> for Duration {
    type Output = Self;

    fn mul(self, rhs: T) -> Self {
        let micros = self.micros.saturating_mul(rhs.into());
        Self { micros }
    }
}

impl<T: Into<u64>> ops::MulAssign<T> for Duration {
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        *self = *self * rhs;
    }
}

impl<T: Into<u64>> ops::Div<T> for Duration {
    type Output = Self;

    fn div(self, rhs: T) -> Self {
        let micros = self.micros / rhs.into();
        Self { micros }
    }
}

impl<T: Into<u64>> ops::DivAssign<T> for Duration {
    #[inline]
    fn div_assign(&mut self, rhs: T) {
        *self = *self / rhs;
    }
}

/// A variant of `Instant` that can be safely shared between threads.
pub struct AtomicInstant {
    micros: AtomicU64,
}

impl AtomicInstant {
    #[must_use]
    #[inline]
    /// Creates a new `AtomicInstant`.
    pub const fn new(instant: Instant) -> Self {
        Self {
            micros: AtomicU64::new(instant.total_micros()),
        }
    }

    #[must_use]
    #[inline]
    /// Loads the current value of the `AtomicInstant`.
    ///
    /// See `AtomicU64::load` for details on the `Ordering` parameter and caveats.
    pub fn load(&self, order: Ordering) -> Instant {
        Instant::from_micros(self.micros.load(order))
    }

    #[inline]
    /// Stores a new value into the `AtomicInstant`.
    ///
    /// See `AtomicU64::store` for details on the `Ordering` parameter and caveats.
    pub fn store(&self, instant: Instant, order: Ordering) {
        self.micros.store(instant.total_micros(), order);
    }

    #[inline]
    /// Adds the given duration to the `AtomicInstant`, returning the previous value.
    ///
    /// See `AtomicU64::fetch_add` for details on the `Ordering` parameter and caveats.
    pub fn fetch_add(&self, duration: Duration, order: Ordering) -> Instant {
        let prev_micros = self.micros.fetch_add(duration.total_micros(), order);
        Instant::from_micros(prev_micros)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_instant() {
        let instant = Instant::from_millis(4242);
        assert_eq!(instant.secs(), 4);
        assert_eq!(instant.millis(), 242);
        assert_eq!(instant.total_millis(), 4242);
    }

    #[test]
    fn test_instant_ops() {
        assert_eq!(
            Instant::from_millis(4) + Duration::from_millis(6),
            Instant::from_millis(10)
        );
        assert_eq!(
            Instant::from_millis(7) - Duration::from_millis(5),
            Instant::from_millis(2)
        );
    }

    #[test]
    fn test_duration() {
        let instant = Duration::from_millis(4242);
        assert_eq!(instant.secs(), 4);
        assert_eq!(instant.millis(), 242);
        assert_eq!(instant.total_millis(), 4242);
    }

    #[test]
    fn test_duration_ops() {
        assert_eq!(
            Duration::from_millis(40) + Duration::from_millis(2),
            Duration::from_millis(42)
        );
        assert_eq!(
            Duration::from_millis(42) - Duration::from_millis(40),
            Duration::from_millis(2)
        );
        assert_eq!(Duration::from_millis(6) * 7_u64, Duration::from_millis(42));
        assert_eq!(
            Duration::from_micros(6595) / 157_u64,
            Duration::from_micros(42)
        );
    }

    #[test]
    fn test_duration_overflow() {
        let overflow = Duration::MAX + Duration::from_millis(1);
        assert_eq!(overflow, Duration::MAX);
    }
    #[test]
    fn test_duration_underflow() {
        let underflow = Duration::ZERO - Duration::from_millis(1);
        assert_eq!(underflow, Duration::ZERO);
    }

    #[test]
    fn test_instant_atomic() {
        let atomic_instant = AtomicInstant::new(Instant::from_millis(1000));
        assert_eq!(
            atomic_instant.load(Ordering::Relaxed),
            Instant::from_millis(1000)
        );
        atomic_instant.store(Instant::from_millis(2000), Ordering::Relaxed);
        assert_eq!(
            atomic_instant.load(Ordering::Relaxed),
            Instant::from_millis(2000)
        );
        let prev = atomic_instant.fetch_add(Duration::from_millis(500), Ordering::Relaxed);
        assert_eq!(prev, Instant::from_millis(2000));
        assert_eq!(
            atomic_instant.load(Ordering::Relaxed),
            Instant::from_millis(2500)
        );
    }
}
