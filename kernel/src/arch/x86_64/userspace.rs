use beskar_core::process::ThreadStartBlock;
use beskar_hal::registers::Rflags;

/// Enter usermode.
///
/// # Safety
///
/// The given stack pointer must be valid, i.e. the stack must be big enough as well as user accessible.
/// The given entrypoint should point to valid, user accessible code.
#[unsafe(naked)]
pub unsafe extern "C" fn enter_usermode(
    entry: extern "C" fn(*const ThreadStartBlock),
    rsp: *mut u8,
    start_block: *const ThreadStartBlock,
) -> ! {
    // RDI contains a pointer to the entry point
    // RSI contains a pointer to the stack pointer
    // RDX contains a pointer to `ThreadStartBlock`
    core::arch::naked_asm!(
        "mov rcx, rdi",
        "mov rdi, rdx",
        "mov r11, {}",
        "cli",
        "lea rsp, [rsi-8]", // Align the stack to 8 mod 16
        "swapgs",
        "sysretq",
        const Rflags::IF | Rflags::BRKI,
    );
}
