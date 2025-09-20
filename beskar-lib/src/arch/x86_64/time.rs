use beskar_core::time::MILLIS_PER_SEC;

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

#[must_use]
/// Returns the TSC frequency in Hz.
pub fn get_tsc_frequency() -> u64 {
    const MEASURE_TIME_MS: u64 = 100;
    const QUANTUM_PER_SEC: u64 = MILLIS_PER_SEC / MEASURE_TIME_MS;

    let start = read_tsc_fenced();
    crate::sleep(crate::time::Duration::from_millis(MEASURE_TIME_MS));
    let end = read_tsc_fenced();

    let delta = end - start;
    delta * QUANTUM_PER_SEC
}
