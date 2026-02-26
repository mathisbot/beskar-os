#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_panics_doc)]
#![feature(pointer_try_cast_aligned)]

extern crate alloc;

pub mod backend;
pub mod compositor;
