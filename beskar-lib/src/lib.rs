//! Standard library for `BeskarOS`.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

extern crate alloc;

pub use ::beskar_core::syscall::ExitCode;
use ::beskar_core::syscall::Syscall;

mod arch;
use arch::syscalls;
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
    let _ = syscalls::syscall_1(Syscall::Exit, code as u64);
    unsafe { ::core::hint::unreachable_unchecked() }
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
#[allow(clippy::missing_panics_doc)]
#[doc(hidden)]
#[inline]
pub unsafe fn __init() {
    let res = mem::mmap(mem::HEAP_SIZE);
    unsafe { mem::init_heap(res.as_ptr(), mem::HEAP_SIZE.try_into().unwrap()) };
}
