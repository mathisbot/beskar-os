use crate::drivers::{hpet, tsc};
pub use beskar_core::time::Duration;
use beskar_core::time::TimerInfo;
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
        let raw = tsc::ticks_per_ms();
        NonZeroU64::new(raw)
    } else if HPET_AVAILABLE.load(Ordering::Acquire) {
        let raw = hpet::ticks_per_ms()?;
        Some(NonZeroU64::from(raw))
    } else {
        None
    }
}

#[must_use]
#[inline]
/// Whether there is a high-precision timer available from userspace.
fn us_fastpath() -> bool {
    cfg_select! {
        target_arch = "x86_64" => TSC_AVAILABLE.load(Ordering::Acquire),
        _ => false,
    }
}

#[must_use]
#[inline]
pub fn timer_info() -> Option<&'static TimerInfo> {
    TIMER_INFO.get()
}
