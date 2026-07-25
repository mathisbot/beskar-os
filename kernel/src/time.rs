use crate::drivers::{hpet, tsc};
pub use beskar_core::time::Duration;
use beskar_core::time::{TICKS_PER_MILLI, TimerInfo};
use core::{
    num::NonZeroU64,
    sync::atomic::{AtomicBool, Ordering},
};
use hyperdrive::once::Once;

mod instant;
pub use instant::Instant;

static HPET_AVAILABLE: AtomicBool = AtomicBool::new(false);
static TSC_AVAILABLE: AtomicBool = AtomicBool::new(false);

static TIMER_INFO: Once<TimerInfo> = Once::uninit();

struct HpetClock;
struct TscClock;

pub fn init() {
    let hpet_res = crate::drivers::hpet::init();
    HPET_AVAILABLE.store(hpet_res.is_ok(), Ordering::Relaxed);
    let tsc_res = crate::drivers::tsc::init();
    TSC_AVAILABLE.store(tsc_res.is_ok(), Ordering::Relaxed);

    let timer_info = TimerInfo {
        ticks_per_ms: ticks_per_ms(),
        fastpath: us_fastpath(),
    };
    TIMER_INFO.call_once(|| timer_info);
}

/// Waits for AT LEAST the given number of milliseconds.
///
/// The real amount of time waited is usually longer than the given duration.
pub fn wait(duration: Duration) {
    if TSC_AVAILABLE.load(Ordering::Acquire) {
        TscClock.wait(duration);
    } else if HPET_AVAILABLE.load(Ordering::Acquire) {
        HpetClock.wait(duration);
    }
}

#[must_use]
#[inline]
pub fn now_raw() -> u64 {
    if TSC_AVAILABLE.load(Ordering::Acquire) {
        tsc::main_counter_value()
    } else if HPET_AVAILABLE.load(Ordering::Acquire) {
        hpet::main_counter_value()
    } else {
        0
    }
}

#[must_use]
#[inline]
pub fn ticks_per_ms() -> Option<NonZeroU64> {
    if TSC_AVAILABLE.load(Ordering::Acquire) {
        Some(TscClock.ticks_per_ms())
    } else if HPET_AVAILABLE.load(Ordering::Acquire) {
        Some(HpetClock.ticks_per_ms())
    } else {
        None
    }
}

#[must_use]
#[inline]
/// Whether there is a high-precision timer available from userspace.
fn us_fastpath() -> bool {
    cfg!(target_arch = "x86_64") && TSC_AVAILABLE.load(Ordering::Acquire)
}

#[must_use]
#[inline]
pub fn timer_info() -> Option<&'static TimerInfo> {
    TIMER_INFO.get()
}

trait Clock {
    #[must_use]
    fn now(&self) -> u64;
    #[must_use]
    fn ticks_per_ms(&self) -> NonZeroU64;
    fn wait(&self, duration: Duration) {
        let start = unsafe { beskar_core::time::Instant::from_raw(self.now()) };
        let end = start
            .checked_add_duration(duration, self.ticks_per_ms().get() * TICKS_PER_MILLI)
            .expect("Overflow when adding duration to instant");
        while self.now() < end.to_raw() {
            core::hint::spin_loop();
        }
    }
}

impl Clock for HpetClock {
    #[inline]
    fn now(&self) -> u64 {
        hpet::main_counter_value()
    }

    #[inline]
    fn ticks_per_ms(&self) -> NonZeroU64 {
        let raw = u64::from(hpet::ticks_per_ms().unwrap().get());
        unsafe { NonZeroU64::new_unchecked(raw) }
    }
}

impl Clock for TscClock {
    #[inline]
    fn now(&self) -> u64 {
        tsc::main_counter_value()
    }

    #[inline]
    fn ticks_per_ms(&self) -> NonZeroU64 {
        let raw = tsc::ticks_per_ms();
        unsafe { NonZeroU64::new_unchecked(raw) }
    }
}
