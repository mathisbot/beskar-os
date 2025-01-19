#[naked]
/// Switches the current stack and CR3 to the ones provided.
///
/// ## Safety
///
/// Interrupts must be disabled when calling this function.
/// This function will re-enable interrupts before returning.
pub unsafe extern "C" fn switch(old_stack: *mut usize, new_stack: *const usize, cr3: usize) {
    // Thanks to the C calling convention,
    // the arguments are in the correct registers:
    // RDI = old_stack
    // RSI = new_stack
    // RDX = cr3

    // FIXME: SIMD regs?
    unsafe {
        core::arch::naked_asm!(
            // Push the current context to the stack
            r#"
            pushfq
            push rax
            push rcx
            push rdx
            push rbx
            sub rsp, {}
            push rbp
            push rsi
            push rdi
            push r8
            push r9
            push r10
            push r11
            push r12
            push r13
            push r14
            push r15
            "#,
            // Update stack pointer
            r#"
            mov [rdi], rsp
            mov rsp, rsi
            "#,
            // Set TS bit in CR0
            r#"
            mov rax, cr0
            or rax, 8
            mov cr0, rax
            "#,
            // Load the new CR3
            r#"
            mov cr3, rdx
            "#,
            // Load the new context from the stack
            r#"
            pop r15
			pop r14
			pop r13
			pop r12
			pop r11
			pop r10
			pop r9
			pop r8
			pop rdi
			pop rsi
			pop rbp
			add rsp, {}
			pop rbx
			pop rdx
			pop rcx
			pop rax
			popfq
            "#,
            // Finally, return to the new stack
            r#"
            sti
            ret
            "#,
            const { size_of::<usize>() },
            const { size_of::<usize>() },
        );
    }
}
