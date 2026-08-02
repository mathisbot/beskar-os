use beskar_core::time::{Duration, Instant as InstantCore, MILLIS_PER_SEC};
use core::ops;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(InstantCore);

impl Instant {
    #[must_use]
    #[inline]
    pub fn now() -> Self {
        let raw = super::now_raw();
        Self(unsafe { InstantCore::from_raw(raw) })
    }

    #[must_use]
    #[inline]
    pub fn elapsed(self) -> Duration {
        let now = Self::now();
        if now < self {
            Duration::ZERO
        } else {
            now - self
        }
    }

    #[must_use]
    #[inline]
    pub(crate) const fn as_inner(self) -> InstantCore {
        self.0
    }
}

impl ops::Add<Duration> for Instant {
    type Output = Self;

    #[expect(clippy::suspicious_arithmetic_impl)]
    fn add(self, rhs: Duration) -> Self {
        let freq = super::ticks_per_ms()
            .expect("No high-precision timer available")
            .get()
            * MILLIS_PER_SEC;
        let inner = self
            .0
            .checked_add_duration(rhs, freq)
            .expect("Overflow when adding duration to instant");
        Self(inner)
    }
}

impl ops::AddAssign<Duration> for Instant {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

impl ops::Sub<Self> for Instant {
    type Output = Duration;

    #[expect(clippy::suspicious_arithmetic_impl)]
    fn sub(self, rhs: Self) -> Duration {
        let freq = super::ticks_per_ms()
            .expect("No high-precision timer available")
            .get()
            * MILLIS_PER_SEC;
        self.0
            .checked_duration_since(rhs.0, freq)
            .expect("Overflow when subtracting instants")
    }
}
