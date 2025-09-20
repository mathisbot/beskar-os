pub use beskar_core::time::{Duration, Instant, MILLIS_PER_SEC};
use hyperdrive::once::Once;

static STARTUP_TIME: Once<Instant> = Once::uninit();

#[must_use]
/// Reads the time in milliseconds since an arbitrary point in the past.
fn read_time_raw() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        static FREQ: Once<u64> = Once::uninit();
        FREQ.call_once(crate::arch::time::get_tsc_frequency);
        let freq = *FREQ.get().unwrap();
        let tsc = crate::arch::time::read_tsc_fenced();
        tsc * MILLIS_PER_SEC / freq
    }

    #[cfg(not(any(target_arch = "x86_64")))]
    {
        unimplemented!("Time reading not implemented for this architecture");
    }
}

/// Initializes the time module.
pub(crate) fn init() {
    STARTUP_TIME.call_once(|| Instant::from_millis(read_time_raw()));
}

#[must_use]
#[inline]
/// Returns the current instant.
pub fn now() -> Instant {
    Instant::from_millis(read_time_raw())
}
