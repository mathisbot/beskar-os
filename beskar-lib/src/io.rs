pub fn print(msg: &str) {
    let res_code: usize;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") 0, // TODO: Automatically determine syscall number
            lateout("rax") res_code,
            in("rdi") msg.as_ptr(),
            in("rsi") msg.len(),
        )
    };
    assert_eq!(res_code, 0); // TODO: Automatically match syscall exit code
}
