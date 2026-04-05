//! Core functionality for Beskar OS.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::missing_safety_doc,
    clippy::doc_markdown
)]

pub mod arch;
pub mod drivers;
pub mod mem;
pub mod process;
pub mod storage;
pub mod syscall;
pub mod time;
pub mod video;

#[macro_export]
/// Compile-time assertion macro.
macro_rules! static_assert {
    ($($arg:tt)*) => {
        const _: () = assert!($($arg)*);
    };
}

#[macro_export]
/// Debug-only panic macro.
macro_rules! debug_panic {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            panic!($($arg)*);
        }
    };
}
