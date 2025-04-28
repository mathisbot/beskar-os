use beskar_core::time::{Duration, Instant};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::drivers::{hpet, tsc};

static HPET_AVAILABLE: AtomicBool = AtomicBool::new(false);
static TSC_AVAILABLE: AtomicBool = AtomicBool::new(false);

struct HpetClock;
struct TscClock;

pub fn init() {
    let hpet_res = crate::drivers::hpet::init();
    HPET_AVAILABLE.store(hpet_res.is_ok(), Ordering::Release);
    let tsc_res = crate::drivers::tsc::init();
    TSC_AVAILABLE.store(tsc_res.is_ok(), Ordering::Release);
}

/// Waits for AT LEAST the given number of milliseconds.
///
/// The real amount of time waited is usually longer than the given duration.
pub fn wait(duration: Duration) {
    if TSC_AVAILABLE.load(Ordering::Acquire) {
        TscClock.wait(duration);
    } else if HPET_AVAILABLE.load(Ordering::Acquire) {
        HpetClock.wait(duration);
    } else {
        panic!("No timer available");
    }
}

trait Clock {
    #[must_use]
    fn now(&self) -> Instant;
    #[must_use]
    fn ticks_per_ms(&self) -> u64;
    fn wait(&self, duration: Duration) {
        let end = self.now() + duration;
        while self.now() < end {
            // TODO: Yield?
            core::hint::spin_loop();
        }
    }
}

impl Clock for HpetClock {
    #[inline]
    fn now(&self) -> Instant {
        Instant::from_millis(hpet::main_counter_value() / self.ticks_per_ms())
    }

    #[inline]
    fn ticks_per_ms(&self) -> u64 {
        u64::from(hpet::ticks_per_ms().unwrap().get())
    }
}

impl Clock for TscClock {
    #[inline]
    fn now(&self) -> Instant {
        Instant::from_millis(tsc::main_counter_value() / self.ticks_per_ms())
    }

    #[inline]
    fn ticks_per_ms(&self) -> u64 {
        tsc::ticks_per_ms()
    }
}
