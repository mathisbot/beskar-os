use crate::arch::time::read_tsc;
use beskar_core::time::{Duration, Instant as InstantCore, TICKS_PER_MILLI, TimerInfo};
use core::mem::MaybeUninit;
use core::ops;
use hyperdrive::once::Once;

#[must_use]
/// Reads the precision counter as a raw value.
///
/// Returns `None` if no high-precision timer is available.
fn query_counter(info: &TimerInfo) -> Option<u64> {
    if info.fastpath {
        cfg_select! {
            target_arch = "x86_64" => {
                Some(read_tsc())
            }
            _ => None
        }
    } else {
        crate::sys::sc_precision_timer()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(InstantCore);

impl Instant {
    #[must_use]
    #[inline]
    /// Returns an `Instant` corresponding to the current time.
    ///
    /// # Panics
    ///
    /// Panics if no high-precision timer is available.
    pub fn now() -> Self {
        let raw = timer_info()
            .and_then(query_counter)
            .expect("No high-precision timer available");
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
}

impl ops::Add<Duration> for Instant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self {
        let freq = timer_info()
            .and_then(|info| info.ticks_per_ms.map(|ticks| ticks.get() * TICKS_PER_MILLI))
            .expect("No high-precision timer available");
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

    fn sub(self, rhs: Self) -> Duration {
        let freq = timer_info()
            .and_then(|info| info.ticks_per_ms.map(|ticks| ticks.get() * TICKS_PER_MILLI))
            .expect("No high-precision timer available");
        self.0
            .checked_duration_since(rhs.0, freq)
            .expect("Overflow when subtracting instants")
    }
}

static TIMER_INFO: Once<TimerInfo> = Once::uninit();

fn timer_info() -> Option<&'static TimerInfo> {
    if let Some(info) = TIMER_INFO.get() {
        return Some(info);
    }

    timer_info_init()
}

#[cold]
fn timer_info_init() -> Option<&'static TimerInfo> {
    let mut uninit = MaybeUninit::<TimerInfo>::uninit();

    let res = crate::sys::sc_query_config(
        beskar_core::syscall::consts::QUERY_HIGH_PRES_TIMER,
        uninit.as_mut_ptr().cast(),
        size_of::<TimerInfo>().try_into().unwrap(),
    );

    if res.is_success() {
        TIMER_INFO.call_once(|| {
            // SAFETY: The syscall initialized the value.
            unsafe { uninit.assume_init() }
        });
    }

    TIMER_INFO.get()
}
