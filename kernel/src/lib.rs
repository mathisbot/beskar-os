#![feature(abi_x86_interrupt, naked_functions)]
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_panics_doc, clippy::similar_names)]

mod arch;
pub mod boot;
pub mod drivers;
pub mod fs;
pub mod locals;
pub mod log;
mod mem;
pub mod process;
pub mod screen;
pub mod time;

extern crate alloc;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    arch::interrupts::int_disable();

    crate::error!(
        "[PANIC]: Core {} {}",
        locals!().core_id(),
        panic_info.message()
    );
    #[cfg(debug_assertions)]
    if let Some(location) = panic_info.location() {
        crate::error!("  at {}", location);
    }

    loop {
        crate::arch::halt();
    }
}
