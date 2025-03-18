use beskar_core::syscall::{Syscall, SyscallExitCode};

pub fn init() {
    crate::arch::syscall::init_syscalls();
}

#[derive(Debug, Copy, Clone)]
pub struct Arguments {
    pub one: u64,
    pub two: u64,
    pub three: u64,
    pub four: u64,
    pub five: u64,
    pub six: u64,
}

#[must_use]
pub fn syscall(syscall: Syscall, args: &Arguments) -> SyscallExitCode {
    match syscall {
        Syscall::Print => sc_print(args),
        Syscall::Exit => sc_exit(args),
        Syscall::RandomGen => sc_randomgen(args),
        Syscall::Invalid => SyscallExitCode::Failure,
    }
}

#[must_use]
fn sc_print(args: &Arguments) -> SyscallExitCode {
    let msg_addr = args.one as *const u8;
    let msg_len = usize::try_from(args.two).unwrap();

    // FIXME: Validate arguments (user is evil)
    // i.e. buffer is in user space, length is valid, etc.

    let buf = unsafe { core::slice::from_raw_parts(msg_addr, msg_len) };
    let Ok(msg) = core::str::from_utf8(buf) else {
        return SyscallExitCode::Failure;
    };

    let tid = crate::process::scheduler::current_thread_id();
    crate::info!("[Thread {}] {}", tid.as_u64(), msg);
    SyscallExitCode::Success
}

fn sc_exit(args: &Arguments) -> ! {
    #[cfg_attr(not(debug_assertions), allow(unused_variables))]
    let exit_code = args.one;

    #[cfg(debug_assertions)]
    {
        let exit_code = beskar_core::syscall::ExitCode::try_from(exit_code);
        let tid = crate::process::scheduler::current_thread_id();

        if let Ok(exit_code) = exit_code {
            crate::debug!("Thread {} exited with code {:?}", tid.as_u64(), exit_code);
        } else {
            crate::debug!("Thread {} exited with invalid code", tid.as_u64());
        }
    }

    unsafe { crate::process::scheduler::exit_current_thread() }
}

fn sc_randomgen(args: &Arguments) -> SyscallExitCode {
    let start_addr = args.one as *mut u8;
    let len = args.two;

    // FIXME: Validate arguments

    let buffer = unsafe { core::slice::from_raw_parts_mut(start_addr, len.try_into().unwrap()) };

    let rand_res = crate::arch::rand::rand_bytes(buffer);

    match rand_res {
        Ok(()) => SyscallExitCode::Success,
        Err(_) => SyscallExitCode::Failure,
    }
}
