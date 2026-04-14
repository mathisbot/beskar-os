pub mod syscalls;
pub mod time;

#[inline]
/// In debug builds, triggers a breakpoint interrupt (`int3`).
pub fn debug_break() {
    if cfg!(debug_assertions) {
        unsafe {
            core::arch::asm!("int3", options(nomem, nostack, preserves_flags));
        }
    }
}

#[inline]
/// In debug builds, triggers a breakpoint interrupt (`int3`).
///
/// The provided value `x` is placed in the `RAX` register before triggering the interrupt.
pub fn debug_break_value(x: u64) {
    if cfg!(debug_assertions) {
        unsafe {
            core::arch::asm!("int3", in("rax") x, options(nomem, nostack, preserves_flags));
        }
    }
}
