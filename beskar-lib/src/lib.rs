//! Standard library for `BeskarOS`.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

extern crate alloc;

pub use beskar_core::syscall::ExitCode;
use beskar_core::{syscall::SyscallExitCode, time::Duration};
use hyperdrive::call_once;

mod arch;
pub mod error;
use error::{SyscallError, SyscallResult};
pub mod io;
pub mod mem;
pub mod prelude;
pub mod rand;
mod sys;
pub mod time;

#[panic_handler]
fn panic(info: &::core::panic::PanicInfo) -> ! {
    println!("Panic occurred: {}", info);
    sys::sc_exit(ExitCode::Failure);
}

#[cold]
/// Exit the program with the given exit code.
pub fn exit(code: ExitCode) -> ! {
    sys::sc_exit(code)
}

#[inline]
/// Sleep for **at least** the given duration.
///
/// # Errors
///
/// Returns an error if the syscall fails.
pub fn sleep(duration: Duration) -> SyscallResult<()> {
    let code = sys::sc_sleep(duration.total_millis());
    match code {
        SyscallExitCode::Success => Ok(()),
        _ => Err(SyscallError::new(-1)),
    }
}

#[macro_export]
/// Sets the entry point for the program.
macro_rules! entry_point {
    ($path:path) => {
        #[macro_use]
        extern crate alloc;

        #[unsafe(export_name = "_start")]
        /// # Safety
        ///
        /// Do not call this function.
        unsafe extern "C" fn __program_entry() {
            $crate::__init();
            ($path)();
            $crate::exit($crate::ExitCode::Success);
        }
    };
}

/// Initialize the standard library.
#[doc(hidden)]
pub fn __init() {
    call_once!({
        // Heap
        {
            let heap_size = mem::HEAP_SIZE;
            let res = mem::mmap(heap_size, None, mem::MemoryProtection::ReadWrite)
                .expect("Memory mapping failed");
            unsafe { mem::init_heap(res.as_ptr(), heap_size.try_into().unwrap()) };
        }

        // Time
        time::init();
    });
}

#[inline]
/// In debug builds, triggers a breakpoint interrupt (`int3`).
pub fn debug_break() {
    if cfg!(debug_assertions) {
        unsafe {
            core::arch::asm!("int3", options(nomem, nostack, preserves_flags));
        }
    }
}

#[inline]
/// In debug builds, triggers a breakpoint interrupt (`int3`).
///
/// The provided value `x` is placed in the `RAX` register before triggering the interrupt.
pub fn debug_break_value(x: u64) {
    if cfg!(debug_assertions) {
        unsafe {
            core::arch::asm!("int3", in("rax") x, options(nomem, nostack, preserves_flags));
        }
    }
}
