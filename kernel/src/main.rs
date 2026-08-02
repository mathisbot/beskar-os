#![no_main]
#![no_std]

extern crate alloc;

use alloc::{boxed::Box, sync::Arc};
use beskar_core::process::perms::Permissions;
use hyperdrive::call_once;
use kernel::{
    locals,
    process::{
        Process,
        scheduler::{
            self, Priority,
            thread::{Thread, start_user_process},
        },
    },
    storage::vfs,
};
use storage::fs::{Path, PathBuf, in_mem::InMemoryFS};

kernel::kernel_main!(kmain);

/// The kernel main function, where every core ends up after initialization
///
/// BSP entry point (called by bootloader) is defined in `boot.rs`.
fn kmain() -> ! {
    if locals!().core_id() == 0 {
        kernel::info!("Welcome to BeskarOS kernel!");
    }

    scheduler::set_scheduling(true);

    call_once!({
        let driver_proc = Arc::new(Process::new(
            "Drivers",
            beskar_hal::process::Kind::Driver,
            None,
            Permissions::all(),
        ));
        Thread::builder(driver_proc, kernel::drivers::init)
            .priority(Priority::Normal)
            .stack_heap(alloc::vec![0; 1024 * 128])
            .spawn();

        if let Some(ramdisk) = kernel::boot::ramdisk()
            && let Ok(ramfs) = InMemoryFS::new(ramdisk)
            && vfs()
                .mount(PathBuf::new("/ramdisk"), Box::new(ramfs))
                .is_ok()
            && let Ok(ram_files) = vfs().read_dir(Path::new("/ramdisk/"))
        {
            for file in ram_files {
                let full_path = PathBuf::new("/ramdisk").join(file.as_path().as_str());
                kernel::info!(
                    "Starting user process for file: {}",
                    full_path.as_path().as_str()
                );
                let user_proc = Arc::new(Process::new(
                    "User",
                    beskar_hal::process::Kind::User,
                    Some(full_path),
                    Permissions::us_root(),
                ));
                Thread::builder_with_arg(user_proc, start_user_process, 64 * 1024)
                    .priority(Priority::Realtime)
                    .stack_heap(alloc::vec![0; 1024 * 16])
                    .spawn();
            }
        } else {
            kernel::warn!("No ramdisk found or failed to mount, skipping user processes");
        }
    });

    unsafe { kernel::process::scheduler::exit_current_thread() }
}
