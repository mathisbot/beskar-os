//! Core functionality for Beskar OS.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::missing_safety_doc
)]

pub mod arch;
pub mod boot;
pub mod drivers;
pub mod mem;
pub mod syscall;
pub mod video;

#[macro_export]
macro_rules! static_assert {
    ($condition:expr $(, $($arg:tt)+)?) => {
        const _: () = assert!($condition $(, $($arg)+)?);
    };
}
