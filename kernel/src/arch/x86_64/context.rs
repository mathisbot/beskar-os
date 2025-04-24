use beskar_core::arch::x86_64::registers::Cr0;

#[unsafe(naked)]
/// Switches the current stack and CR3 to the ones provided.
///
/// ## Safety
///
/// Interrupts must be disabled when calling this function.
/// This function will re-enable interrupts before returning.
pub unsafe extern "C" fn switch(old_stack: *mut *mut u8, new_stack: *const u8, cr3: u64) {
    // Thanks to the C calling convention,
    // the arguments are in the correct registers:
    // RDI = old_stack
    // RSI = new_stack
    // RDX = cr3
    core::arch::naked_asm!(
        // Push the current context to the stack
        "pushfq",
        "push rax",
        "push rcx",
        "push rdx",
        "push rbx",
        "push rbp",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        // Update stack pointer
        "mov [rdi], rsp",
        "mov rsp, rsi",
        // Set TS bit in CR0
        "mov rax, cr0",
        "or rax, {ts}",
        "mov cr0, rax",
        // Check if CR3 is different
        "mov rax, cr3",
        "cmp rax, rdx",
        "je 2f",
        // Load the new CR3 ONLY if it is different
        "mov cr3, rdx",
        "2:",
        // Load the new context from the stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rbp",
        "pop rbx",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "popfq",
        // Finally, return to the new stack
        "sti",
        "ret",
        ts = const Cr0::TASK_SWITCHED,
    );
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
/// Registers that are relevant for the thread context.
pub struct ThreadRegisters {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rbx: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
    rflags: u64,
    rip: u64,
}

impl ThreadRegisters {
    #[must_use]
    #[inline]
    pub const fn new(rflags: u64, rip: u64, rbp: u64) -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rdi: 0,
            rsi: 0,
            rbp,
            rbx: 0,
            rdx: 0,
            rcx: 0,
            rax: 0,
            rflags,
            rip,
        }
    }
}
