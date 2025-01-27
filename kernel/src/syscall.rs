use beskar_core::syscall::{Syscall, SyscallExitCode};

pub fn init() {
    crate::arch::syscall::init_syscalls();
}

#[derive(Debug, Copy, Clone)]
pub struct Arguments {
    pub one: u64,
    pub two: u64,
    pub three: u64,
}

pub fn syscall(syscall: Syscall, args: &Arguments) -> SyscallExitCode {
    match syscall {
        Syscall::Print => sc_print(args),
        Syscall::Exit => sc_exit(args),
        Syscall::Invalid => SyscallExitCode::Failure,
    }
}

fn sc_print(args: &Arguments) -> SyscallExitCode {
    let msg_addr = args.one as *const u8;
    let msg_len = usize::try_from(args.two).unwrap();

    let msg =
        unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(msg_addr, msg_len)) };

    let tid = crate::process::scheduler::current_thread_id();
    crate::info!("[Thread {}] {}", tid.as_u64(), msg);
    SyscallExitCode::Success
}

fn sc_exit(args: &Arguments) -> ! {
    // TODO: Use exit code
    let _exit_code = args.one;

    unsafe { crate::process::scheduler::exit_current_thread() };
    unreachable!()
}
