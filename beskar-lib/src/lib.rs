//! Standard library for `BeskarOS`.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

extern crate alloc;

pub use beskar_core::{process::ThreadStartBlock, syscall::ExitCode};
use beskar_core::{
    process::{SleepHandle, WaitResult},
    time::Duration,
};
use core::sync::atomic::{AtomicBool, Ordering};
use hyperdrive::call_once;

mod arch;
pub mod error;
use error::{SyscallError, SyscallResult};
pub mod io;
pub mod mem;
pub mod prelude;
pub mod rand;
pub mod surface;
pub mod sync;
mod sys;
pub mod time;

static PANIC_NESTED: AtomicBool = AtomicBool::new(false);

/// Returns `true` if the current thread is already panicking (i.e., if we're in a nested panic).
pub fn panicking() -> bool {
    PANIC_NESTED.load(Ordering::Acquire)
}

#[panic_handler]
fn panic(info: &::core::panic::PanicInfo) -> ! {
    if !panicking() {
        PANIC_NESTED.store(true, Ordering::Release);
        println!("Panic occurred: {}", info);
    }

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
    let code = sys::sc_wait_on_event(SleepHandle::NONE, duration.total_micros());
    match code {
        WaitResult::Timeout => Ok(()),
        _ => Err(SyscallError::new(-1)),
    }
}

#[macro_export]
/// Sets the entry point for the program.
///
/// The target function must have the signature:
/// `fn(&beskar_lib::ThreadStartBlock)`.
macro_rules! entry_point {
    ($path:path) => {
        #[macro_use]
        extern crate alloc;

        #[unsafe(export_name = "_start")]
        /// # Safety
        ///
        /// Do not call this function.
        unsafe extern "C" fn __program_entry(start_block: *const $crate::ThreadStartBlock) -> ! {
            $crate::__init();

            let start_block = unsafe { &*start_block };
            ($path)(start_block);

            $crate::exit($crate::ExitCode::Success);
        }
    };
}

/// Initialize the standard library.
#[doc(hidden)]
pub fn __init() {
    call_once!({
        // Heap
        mem::init_heap();
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
