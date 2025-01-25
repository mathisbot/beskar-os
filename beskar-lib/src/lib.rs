//! Standard library for BeskarOS.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
pub mod io;

pub fn exit(_code: usize) -> ! {
    loop {
        // TODO: Exit syscall
    }
}
