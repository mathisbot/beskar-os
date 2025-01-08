#![no_main]
#![no_std]
#![warn(clippy::pedantic, clippy::nursery)]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("BeskarOS kernel only supports x86_64 architecture");

use hyperdrive::{once::Once, sync::barrier::Barrier};
use kernel::{locals, process::scheduler};
use x86_64::instructions::hlt;

kernel::kernel_main!(kmain);

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    x86_64::instructions::interrupts::disable();

    kernel::error!("[PANIC]: Core {} {}", locals!().core_id(), panic_info);
    kernel::error!(
        "[PANIC]: Core {} {}",
        locals!().core_id(),
        panic_info.message()
    );
    #[cfg(debug_assertions)]
    if let Some(location) = panic_info.location() {
        kernel::error!("  at {}", location);
    }

    loop {
        hlt();
    }
}

/// The kernel main function, where every core ends up after initialization
///
/// BSP entry point (called by bootloader) is defined in `lib.rs`.
fn kmain() -> ! {
    static BARRIER: Once<Barrier> = Once::uninit();

    if locals!().core_id() == 0 {
        kernel::debug!(
            "Started kernel in {:.1?}",
            kernel::time::tsc::time_since_startup()
        );
        kernel::info!("Welcome to BeskarOS kernel!");
    }

    // TODO: Start user-space processes
    // (GUI, ...)
    BARRIER.call_once(|| Barrier::new(locals::get_ready_core_count().into()));

    scheduler::set_scheduling(false);
    BARRIER.get().unwrap().wait();

    if locals!().core_id() == 0 {
        use kernel::process::{
            dummy,
            scheduler::{self, priority::Priority, thread::Thread},
        };
        extern crate alloc;

        scheduler::spawn_thread(alloc::boxed::Box::pin(Thread::new(
            unsafe { scheduler::current_process() },
            Priority::Normal,
            alloc::vec![0; 1024*256],
            dummy::fibonacci as *const (),
        )));
        scheduler::spawn_thread(alloc::boxed::Box::pin(Thread::new(
            unsafe { scheduler::current_process() },
            Priority::Normal,
            alloc::vec![0; 1024*256],
            dummy::counter as *const (),
        )));
    }
    BARRIER.get().unwrap().wait();
    scheduler::set_scheduling(true);

    // TODO: Stop this thread from being scheduled

    kernel::error!(
        "Kernel main function reached the end on core {}",
        locals!().core_id()
    );

    loop {
        hlt();
    }
}
