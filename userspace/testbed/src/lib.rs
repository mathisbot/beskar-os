#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc)]

extern crate alloc;

pub mod core;
pub mod io;
pub mod mem;
pub mod surface;
pub mod sync;
pub mod thread;
