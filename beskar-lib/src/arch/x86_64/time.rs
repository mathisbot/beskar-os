#[must_use]
#[inline]
pub fn read_tsc_p() -> (u64, u32) {
    cfg_select! {
        target_arch = "x86_64" => {
            let id = 0;
            // let ts = unsafe { core::arch::x86_64::__rdtscp(&raw mut id) }; // TODO: Use RTSCP when available
            unsafe { core::arch::x86_64::_mm_lfence() };
            let ts = unsafe { core::arch::x86_64::_rdtsc() };
            unsafe { core::arch::x86_64::_mm_lfence() };
            (ts, id)
        }
        _ => unimplemented!()
    }
}

#[must_use]
#[inline]
pub fn read_tsc() -> u64 {
    let (ts, _id) = read_tsc_p();
    ts
}
