#![no_main]
#![no_std]

extern crate alloc;

use alloc::{boxed::Box, sync::Arc};
use hyperdrive::once::Once;
use kernel::{
    locals,
    process::{
        Process,
        scheduler::{
            self, Priority,
            thread::{Thread, user_trampoline},
        },
    },
    storage::vfs,
};
use storage::fs::{Path, PathBuf, in_mem::InMemoryFS};

kernel::kernel_main!(kmain);

static SPAWN_ONCE: Once<()> = Once::uninit();

/// The kernel main function, where every core ends up after initialization
///
/// BSP entry point (called by bootloader) is defined in `boot.rs`.
fn kmain() -> ! {
    if locals!().core_id() == 0 {
        video::info!("Welcome to BeskarOS kernel!");
    }

    scheduler::set_scheduling(true);

    // TODO: Start user-space processes
    // (GUI, ...)

    SPAWN_ONCE.call_once(|| {
        let driver_proc = Arc::new(Process::new(
            "Drivers",
            beskar_hal::process::Kind::Driver,
            None,
        ));
        scheduler::spawn_thread(alloc::boxed::Box::new(Thread::new(
            driver_proc,
            Priority::Low,
            alloc::vec![0; 1024 * 128],
            kernel::drivers::init,
        )));

        if let Some(ramdisk) = kernel::boot::ramdisk() {
            let ramfs = InMemoryFS::new(ramdisk).unwrap();
            vfs().mount(PathBuf::new("/ramdisk"), Box::new(ramfs));
            let ram_files = vfs().read_dir(Path::new("/ramdisk/")).unwrap();

            for file in ram_files {
                let full_path = PathBuf::new("/ramdisk").join(file.as_path().as_str());
                video::info!(
                    "Starting user process for file: {}",
                    full_path.as_path().as_str()
                );
                let user_proc = Arc::new(Process::new(
                    "User",
                    beskar_hal::process::Kind::User,
                    Some(full_path),
                ));
                scheduler::spawn_thread(alloc::boxed::Box::new(Thread::new(
                    user_proc,
                    Priority::High,
                    alloc::vec![0; 1024*64],
                    user_trampoline,
                )));
            }
        }
    });

    unsafe { kernel::process::scheduler::exit_current_thread() }
}
