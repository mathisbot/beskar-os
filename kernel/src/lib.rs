#![no_std]
#![allow(
    clippy::missing_panics_doc,
    clippy::similar_names,
    clippy::missing_errors_doc,
    clippy::doc_markdown
)]

use core::fmt::Display;

extern crate alloc;

mod arch;
pub mod boot;
pub mod drivers;
pub mod locals;
mod mem;
pub mod network;
pub mod panic;
pub mod power;
pub mod process;
pub mod storage;
mod syscall;
mod time;
pub mod trace;
mod video;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    let msg: &dyn Display = if cfg!(debug_assertions) {
        &panic_info
    } else {
        &panic_info.message()
    };
    panic::panic_entry(msg);
}
