/// Enter usermode
///
/// ## Safety
///
/// The stack must be set up correctly before calling this function.
/// This function should be called by a kernel thread to enter usermode.
#[naked]
pub unsafe extern "C" fn enter_usermode(entry: extern "C" fn()) {
    // RDI contains a pointer to the entry point
    unsafe {
        core::arch::naked_asm!("mov rcx, rdi", "pushfq", "pop r11", "sysretq");
    }
}
