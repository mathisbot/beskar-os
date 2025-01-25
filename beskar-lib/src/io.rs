pub fn print(msg: &str) {
    unsafe {
        core::arch::asm!(
            "mov rax, {0:r}",
            "syscall",
            in(reg) 0, // TODO: Automatically determine syscall number
            in("rdi") msg.as_ptr(),
            in("rsi") msg.len(),
        )
    };
}
