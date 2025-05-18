//! Standard library for `BeskarOS`.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

extern crate alloc;

use beskar_core::syscall::{Syscall, SyscallExitCode};
pub use beskar_core::{syscall::ExitCode, time::Duration};

mod arch;
pub mod io;
pub mod mem;
pub mod rand;

#[panic_handler]
fn panic(_info: &::core::panic::PanicInfo) -> ! {
    exit(ExitCode::Failure);
}

#[inline]
/// Exit the program with the given exit code.
pub fn exit(code: ExitCode) -> ! {
    let _ = arch::syscalls::syscall_1(Syscall::Exit, code as u64);
    unsafe { ::core::hint::unreachable_unchecked() }
}

#[inline]
/// Sleep for **at least** the given duration.
///
/// # Panics
///
/// Panics if the syscall fails (should never happen).
pub fn sleep(duration: Duration) {
    let res = arch::syscalls::syscall_1(Syscall::Sleep, duration.total_millis());
    assert_eq!(
        SyscallExitCode::try_from(res).unwrap_or(SyscallExitCode::Failure),
        SyscallExitCode::Success
    );
}

#[macro_export]
/// Sets the entry point for the program.
macro_rules! entry_point {
    ($path:path) => {
        extern crate alloc;

        #[unsafe(export_name = "_start")]
        pub extern "C" fn __program_entry() {
            unsafe { $crate::__init() };
            ($path)();
            $crate::exit($crate::ExitCode::Success);
        }
    };
}

/// Initialize the standard library.
///
/// ## Safety
///
/// Do not call this function.
#[doc(hidden)]
pub unsafe fn __init() {
    let res = mem::mmap(mem::HEAP_SIZE, None);
    unsafe { mem::init_heap(res.as_ptr(), mem::HEAP_SIZE.try_into().unwrap()) };
}
