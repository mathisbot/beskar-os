use beskar_core::arch::x86_64::registers::Cr0;

#[naked]
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

    unsafe {
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
}
