#![no_main]
#![no_std]
#![warn(clippy::pedantic, clippy::nursery)]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("BeskarOS kernel only supports x86_64 architecture");

use kernel::locals;
use x86_64::instructions::hlt;

kernel::kernel_main!(kmain);

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    x86_64::instructions::interrupts::disable();

    kernel::sererror!("[PANIC]: Core {} {}", locals!().core_id(), panic_info);
    log::error!(
        "[PANIC]: Core {} {}",
        locals!().core_id(),
        panic_info.message()
    );
    #[cfg(debug_assertions)]
    if let Some(location) = panic_info.location() {
        log::error!("  at {}", location);
    }

    loop {
        hlt();
    }
}

/// The kernel main function, where every core ends up after initialization
///
/// BSP entry point (called by bootloader) is defined in `lib.rs`.
fn kmain() -> ! {
    if locals!().core_id() == 0 {
        if log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "Started kernel in {:.1?}",
                kernel::time::tsc::time_since_startup()
            );
        }
        log::info!("Welcome to BeskarOS kernel!");
    }

    // TODO: Start user-space processes
    // (GUI, ...)

    // TODO: Stop this thread from being scheduled

    log::error!(
        "Kernel main function reached the end on core {}",
        locals!().core_id()
    );

    loop {
        hlt();
    }
}
