//! Standard library for BeskarOS.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
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
            in("rax") 1, // TODO: Automatically determine syscall number
            in("rdi") code as usize,
        )
    };
    unsafe { core::hint::unreachable_unchecked() }
}

#[macro_export]
macro_rules! entry_point {
    ($path:path) => {
        #[unsafe(export_name = "_start")]
        pub extern "C" fn __program_entry() {
            ($path)()
        }
    };
}
