#![feature(abi_x86_interrupt, naked_functions)]
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_panics_doc, clippy::similar_names)]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("BeskarOS kernel only supports x86_64 architecture");

pub mod boot;
pub mod cpu;
pub mod drivers;
pub mod locals;
pub mod log;
mod mem;
pub mod pci;
pub mod process;
pub mod screen;
pub mod serial;
mod syscall;
pub mod time;
pub mod video;

extern crate alloc;
