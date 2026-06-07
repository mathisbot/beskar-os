//! Standard library for `BeskarOS`.
#![no_std]

extern crate alloc;

pub use beskar_core::{process::ThreadStartBlock, syscall::ExitCode};
use hyperdrive::call_once;

mod arch;
pub use arch::{debug_break, debug_break_value};
pub mod error;
pub mod io;
pub mod mem;
pub mod power;
pub mod prelude;
pub mod rand;
pub mod surface;
pub mod sync;
mod sys;
pub mod thread;
pub mod time;

#[panic_handler]
fn panic(info: &::core::panic::PanicInfo) -> ! {
    use core::sync::atomic::{AtomicBool, Ordering};

    // FIXME: This is process-wide!
    static PANICKED: AtomicBool = AtomicBool::new(false);
    if !PANICKED.swap(true, Ordering::SeqCst) {
        println!("Panic occurred: {}", info);
    }

    sys::sc_exit(ExitCode::Failure);
}

#[cold]
/// Exit the program with the given exit code.
pub fn exit(code: ExitCode) -> ! {
    sys::sc_exit(code)
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
