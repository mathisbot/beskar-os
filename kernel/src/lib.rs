#![feature(abi_x86_interrupt, naked_functions)]
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_panics_doc, clippy::similar_names)]

mod arch;
pub mod boot;
pub mod drivers;
pub mod locals;
pub mod log;
mod mem;
pub mod network;
pub mod process;
pub mod screen;
pub mod storage;
pub mod syscall;
pub mod time;

extern crate alloc;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    arch::interrupts::int_disable();

    #[cfg(debug_assertions)]
    crate::error!("[PANIC]: Core {} - {}", locals!().core_id(), panic_info);
    #[cfg(not(debug_assertions))]
    crate::error!(
        "[PANIC]: Core {} - {}",
        locals!().core_id(),
        panic_info.message()
    );

    if process::scheduler::is_scheduling_init() {
        // TODO: Check if kernel process -> send NMI and BSOD
        unsafe { process::scheduler::exit_current_thread() };
    }

    loop {
        crate::arch::halt();
    }
}

#[macro_export]
macro_rules! static_assert {
    ($assertion:expr $(, $($arg:tt)+)?) => {
        const _: () = assert!($assertion $(, $($arg)+)?);
    };
}
