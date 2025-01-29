use core::sync::atomic::{AtomicBool, Ordering};
pub use core::time::Duration;

use crate::drivers::{hpet, tsc};

static HPET_AVAILABLE: AtomicBool = AtomicBool::new(false);
static TSC_AVAILABLE: AtomicBool = AtomicBool::new(false);

struct HpetClock;
struct TscClock;

pub fn init() {
    let hpet_res = crate::drivers::hpet::init();
    HPET_AVAILABLE.store(hpet_res.is_ok(), Ordering::Relaxed);
    let tsc_res = crate::drivers::tsc::init();
    TSC_AVAILABLE.store(tsc_res.is_ok(), Ordering::Relaxed);
}

/// Wait AT LEAST the given number of milliseconds.
pub fn wait(duration: Duration) {
    if TSC_AVAILABLE.load(Ordering::Relaxed) {
        TscClock.wait(duration);
    } else if HPET_AVAILABLE.load(Ordering::Relaxed) {
        HpetClock.wait(duration);
    } else {
        panic!("No timer available");
    }
}

trait Clock {
    fn now(&self) -> u64;
    fn ticks_per_ms(&self) -> u64;
    fn wait(&self, duration: Duration) {
        let ms = u64::try_from(duration.as_millis()).expect("Duration too large");
        let end = self.now() + ms * self.ticks_per_ms();
        while self.now() < end {
            core::hint::spin_loop();
        }
    }
}

impl Clock for HpetClock {
    fn now(&self) -> u64 {
        hpet::main_counter_value()
    }

    fn ticks_per_ms(&self) -> u64 {
        hpet::ticks_per_ms()
    }
}

impl Clock for TscClock {
    fn now(&self) -> u64 {
        tsc::main_counter_value()
    }

    fn ticks_per_ms(&self) -> u64 {
        tsc::ticks_per_ms()
    }
}
