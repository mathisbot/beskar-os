/// Enter usermode.
///
/// # Safety
///
/// The given stack pointer must be valid, i.e. the stack must be big enough as well as user accessible.
/// The given entrypoint should point to valid, user accessible code.
/// Also, as a matter of safety, interrupts should be enabled before calling this function,
/// otherwise the CPU will be stuck in usermode!
#[unsafe(naked)]
pub unsafe extern "C" fn enter_usermode(entry: extern "C" fn(), rsp: *mut u8) -> ! {
    // RDI contains a pointer to the entry point
    // RSI contains a pointer to the stack pointer
    core::arch::naked_asm!(
        "mov rcx, rdi",
        "pushfq",
        "pop r11",
        "mov rsp, rsi",
        "sysretq",
    );
}
