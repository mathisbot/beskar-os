/// Enter usermode
///
/// The stack must be set up correctly before calling this function.
/// This function should be called by a kernel thread to enter usermode.
pub unsafe fn enter_usermode(entry: *const ()) {
    unsafe {
        core::arch::asm!(
            "mov rcx, {}",
            "pushfq",
            "pop r11",
            "sysretq",
            in(reg) entry,
        );
    }
}
