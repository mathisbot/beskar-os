use beskar_core::time::{Duration, MILLIS_PER_SEC};

#[must_use]
#[inline]
pub fn read_tsc_fenced() -> u64 {
    unsafe {
        core::arch::x86_64::_mm_mfence();
        let tsc = core::arch::x86_64::_rdtsc();
        core::arch::x86_64::_mm_lfence();
        tsc
    }
}

/// Returns the TSC frequency in Hz.
pub fn get_tsc_frequency() -> crate::error::SyscallResult<u64> {
    const MEASURE_TIME_MS: u64 = 100;
    const QUANTUM_PER_SEC: u64 = MILLIS_PER_SEC / MEASURE_TIME_MS;

    let start = read_tsc_fenced();
    crate::sleep(Duration::from_millis(MEASURE_TIME_MS))?;
    let end = read_tsc_fenced();

    let delta = end - start;
    Ok(delta * QUANTUM_PER_SEC)
}
