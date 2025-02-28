#[inline]
pub unsafe fn load_tss(selector: u16) {
    unsafe {
        core::arch::asm!("ltr {0:x}", in(reg) selector, options(nostack, preserves_flags));
    }
}
