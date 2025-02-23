#![no_main]
#![no_std]

use hyperdrive::once::Once;
use kernel::{locals, process::scheduler};

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

        let root_proc = scheduler::current_process();

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
    });

    unsafe { kernel::process::scheduler::exit_current_thread() };

    kernel::error!(
        "Kernel main function reached the end on core {}",
        locals!().core_id()
    );

    unreachable!()
}
