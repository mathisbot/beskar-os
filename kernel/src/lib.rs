#![feature(abi_x86_interrupt)]
#![no_std]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_panics_doc, clippy::similar_names)]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("BeskarOS kernel only supports x86_64 architecture");

pub mod boot;
mod cpu;
pub mod locals;
mod logging;
mod mem;
mod pci;
mod process;
pub mod screen;
pub mod serial;
mod syscall;
pub mod time;
pub mod utils;

extern crate alloc;
