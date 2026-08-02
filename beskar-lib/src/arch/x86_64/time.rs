#[must_use]
#[inline]
/// Returns the current value of the Time Stamp Counter (TSC)
/// and the timestamp universe ID (for non-invariant TSCs).
pub fn read_tsc_p() -> (u64, u32) {
    let id = 0;
    // let ts = unsafe { core::arch::x86_64::__rdtscp(&raw mut id) }; // TODO: Use RTSCP when available
    unsafe { core::arch::x86_64::_mm_lfence() };
    let ts = unsafe { core::arch::x86_64::_rdtsc() };
    unsafe { core::arch::x86_64::_mm_lfence() };
    (ts, id)
}

#[must_use]
#[inline]
/// Returns the current value of the Time Stamp Counter (TSC).
pub fn read_tsc() -> u64 {
    let (ts, _) = read_tsc_p();
    ts
}
