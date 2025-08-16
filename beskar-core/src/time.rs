use core::{fmt, ops};

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

    #[must_use]
    #[inline]
    /// Create a new `Instant` from a number of microseconds.
    pub const fn from_micros(micros: u64) -> Self {
        Self { micros }
    }
    #[must_use]
    #[track_caller]
    #[inline]
    /// Create a new `Instant` from a number of milliseconds.
    pub const fn from_millis(millis: u64) -> Self {
        let micros = millis
            .checked_mul(MICROS_PER_MILLI)
            .expect("millis overflowed");
        Self::from_micros(micros)
    }
    #[must_use]
    #[track_caller]
    #[inline]
    /// Create a new `Instant` from a number of seconds.
    pub const fn from_secs(secs: u64) -> Self {
        let micros = secs.checked_mul(MICROS_PER_SEC).expect("secs overflowed");
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
        Self::from_micros(
            self.total_micros()
                .checked_add(rhs.total_micros())
                .expect("overflow when adding durations"),
        )
    }
}

impl ops::AddAssign<Duration> for Instant {
    fn add_assign(&mut self, rhs: Duration) {
        self.micros = self
            .total_micros()
            .checked_add(rhs.total_micros())
            .expect("overflow when adding durations");
    }
}

impl ops::Sub<Duration> for Instant {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self {
        Self::from_micros(
            self.total_micros()
                .checked_sub(rhs.total_micros())
                .expect("underflow when subtracting durations"),
        )
    }
}

impl ops::SubAssign<Duration> for Instant {
    fn sub_assign(&mut self, rhs: Duration) {
        self.micros = self
            .total_micros()
            .checked_sub(rhs.total_micros())
            .expect("underflow when subtracting durations");
    }
}

impl ops::Sub<Self> for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Duration {
        Duration::from_micros(self.total_micros().abs_diff(rhs.total_micros()))
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
    #[track_caller]
    #[inline]
    /// Create a new `Duration` from a number of milliseconds.
    pub const fn from_millis(millis: u64) -> Self {
        let micros = millis
            .checked_mul(MICROS_PER_MILLI)
            .expect("millis overflowed");
        Self::from_micros(micros)
    }
    #[must_use]
    #[track_caller]
    #[inline]
    /// Create a new `Duration` from a number of seconds.
    pub const fn from_secs(secs: u64) -> Self {
        let micros = secs.checked_mul(MICROS_PER_SEC).expect("secs overflowed");
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
        Self::from_micros(
            self.total_micros()
                .checked_add(rhs.total_micros())
                .expect("overflow when adding durations"),
        )
    }
}

impl ops::AddAssign<Self> for Duration {
    fn add_assign(&mut self, rhs: Self) {
        self.micros = self
            .total_micros()
            .checked_add(rhs.total_micros())
            .expect("overflow when adding durations");
    }
}

impl ops::Sub<Self> for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::from_micros(
            self.total_micros()
                .checked_sub(rhs.total_micros())
                .expect("underflow when subtracting durations"),
        )
    }
}

impl ops::SubAssign<Self> for Duration {
    fn sub_assign(&mut self, rhs: Self) {
        self.micros = self
            .total_micros()
            .checked_sub(rhs.total_micros())
            .expect("under when subtracting durations");
    }
}

impl<T: Into<u64>> ops::Mul<T> for Duration {
    type Output = Self;

    fn mul(self, rhs: T) -> Self {
        Self::from_micros(
            self.micros
                .checked_mul(<T as Into<u64>>::into(rhs))
                .expect("overflow when multiplying durations"),
        )
    }
}

impl<T: Into<u64>> ops::MulAssign<T> for Duration {
    fn mul_assign(&mut self, rhs: T) {
        self.micros = self
            .total_micros()
            .checked_mul(<T as Into<u64>>::into(rhs))
            .expect("overflow when multiplying durations");
    }
}

impl<T: Into<u64>> ops::Div<T> for Duration {
    type Output = Self;
    fn div(self, rhs: T) -> Self {
        Self::from_micros(
            self.micros
                .checked_div(<T as Into<u64>>::into(rhs))
                .expect("division by zero when dividing durations"),
        )
    }
}

impl<T: Into<u64>> ops::DivAssign<T> for Duration {
    fn div_assign(&mut self, rhs: T) {
        self.micros = self
            .total_micros()
            .checked_div(<T as Into<u64>>::into(rhs))
            .expect("division by zero when dividing durations");
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
    #[should_panic(expected = "overflow when adding durations")]
    fn test_duration_overflow() {
        let _ = Duration::MAX + Duration::from_millis(1);
    }
    #[test]
    #[should_panic(expected = "underflow when subtracting durations")]
    fn test_duration_underflow() {
        let _ = Duration::from_millis(0) - Duration::from_millis(1);
    }
}
