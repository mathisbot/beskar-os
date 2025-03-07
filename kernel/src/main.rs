#![no_main]
#![no_std]

extern crate alloc;

use alloc::sync::Arc;
use hyperdrive::once::Once;
use kernel::{
    locals,
    process::{Process, scheduler},
};

kernel::kernel_main!(kmain);

static SPAWN_ONCE: Once<()> = Once::uninit();

/// The kernel main function, where every core ends up after initialization
///
/// BSP entry point (called by bootloader) is defined in `boot.rs`.
fn kmain() -> ! {
    if locals!().core_id() == 0 {
        kernel::info!("Welcome to BeskarOS kernel!");
    }

    scheduler::set_scheduling(true);

    // TODO: Start user-space processes
    // (GUI, ...)

    SPAWN_ONCE.call_once(|| {
        use kernel::process::{
            dummy,
            scheduler::{self, priority::Priority, thread::Thread},
        };
        extern crate alloc;

        let root_proc = Arc::new(Process::new("Tests", kernel::process::Kind::Driver));

        scheduler::spawn_thread(alloc::boxed::Box::pin(Thread::new(
            root_proc.clone(),
            Priority::Normal,
            alloc::vec![0; 1024*256],
            dummy::fibonacci,
        )));
        scheduler::spawn_thread(alloc::boxed::Box::pin(Thread::new(
            root_proc.clone(),
            Priority::Normal,
            alloc::vec![0; 1024*256],
            dummy::counter,
        )));
        // scheduler::spawn_thread(alloc::boxed::Box::pin(Thread::new(
        //     root_proc.clone(),
        //     Priority::Low,
        //     alloc::vec![0; 1024*1024],
        //     dummy::alloc_intensive,
        // )));
        scheduler::spawn_thread(alloc::boxed::Box::pin(Thread::new(
            root_proc.clone(),
            Priority::Low,
            alloc::vec![0; 1024*256],
            dummy::panic_test,
        )));
        scheduler::spawn_thread(alloc::boxed::Box::pin(Thread::new(
            root_proc.clone(),
            Priority::Normal,
            alloc::vec![0; 1024*256],
            dummy::floating_point,
        )));
        // scheduler::spawn_thread(alloc::boxed::Box::pin(Thread::new(
        //     root_proc.clone(),
        //     Priority::Low,
        //     alloc::vec![0; 1024*256],
        //     dummy::syscall_test,
        // )));

        if let Some(ramdisk) = kernel::boot::ramdisk() {
            let try_load = kernel::process::binary::Binary::new(
                ramdisk,
                kernel::process::binary::BinaryType::Elf,
            )
            .load();
            if let Ok(binary) = try_load {
                scheduler::spawn_thread(alloc::boxed::Box::pin(Thread::new(
                    root_proc.clone(),
                    Priority::Normal,
                    alloc::vec![0; 1024*256],
                    binary.entry_point(),
                )));
                kernel::debug!("Ramdisk loaded!");
            } else {
                kernel::error!("Failed to load ramdisk");
            }
        }
    });

    unsafe { kernel::process::scheduler::exit_current_thread() }
}
