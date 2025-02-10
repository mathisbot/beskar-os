//! Core functionality for Beskar OS.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

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
