//! Standard library for `BeskarOS`.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

use beskar_core::syscall::Syscall;
pub mod io;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(ExitCode::Failure)
}

pub enum ExitCode {
    Success = 0,
    Failure = 1,
}

pub fn exit(code: ExitCode) -> ! {
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") Syscall::Exit as u64,
            in("rdi") code as u64,
            options(noreturn),
        );
    }
}

#[macro_export]
macro_rules! entry_point {
    ($path:path) => {
        #[inline]
        #[unsafe(export_name = "_start")]
        pub extern "C" fn __program_entry() {
            ($path)()
        }
    };
}
