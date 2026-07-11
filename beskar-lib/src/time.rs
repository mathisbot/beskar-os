use crate::arch::time::read_tsc;
use beskar_core::time::TimerInfo;
pub use beskar_core::time::{Duration, Instant};
use core::mem::MaybeUninit;
use hyperdrive::once::Once;

static TIMER_INFO: Once<TimerInfo> = Once::uninit();

#[must_use]
/// Reads the time in milliseconds since an arbitrary point in the past.
fn query_counter() -> Option<Instant> {
    let info = timer_info()?;

    if info.fastpath {
        let raw = cfg_select! {
            target_arch = "x86_64" => {
                read_tsc()
            }
            _ => unimplemented!()
        };
        let freq = info.ticks_per_ms?;
        let millis = raw / freq;
        Some(Instant::from_millis(millis))
    } else {
        crate::sys::sc_precision_timer()
    }
}

#[must_use]
#[inline]
/// Returns the current instant.
///
/// # Panics
///
/// This function panics if no high-precision timer is available.
pub fn now() -> Instant {
    query_counter().expect("No timer available")
}

#[must_use]
#[inline]
/// Returns the duration elapsed since the given instant.
///
/// # Panics
///
/// This function panics if no high-precision timer is available.
pub fn elapsed(since: Instant) -> Duration {
    let now = now();
    if now < since {
        Duration::ZERO
    } else {
        now - since
    }
}

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
