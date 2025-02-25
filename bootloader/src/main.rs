#![no_main]
#![no_std]
#![warn(clippy::pedantic, clippy::nursery)]

use bootloader::{debug, error, info, warn};
use uefi::{mem::memory_map::MemoryMapMut as _, prelude::*};

#[panic_handler]
/// Handles panics in the bootloader by logging an error message and attempting
/// to either gracefully reset the system or hang if necessary.
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("[PANIC]: {}", panic_info.message());

    // If in debug mode, delay to allow the user to read the message
    #[cfg(debug_assertions)]
    {
        if let Some(location) = panic_info.location() {
            error!(
                "Panic occured in file '{}' at line {}",
                location.file(),
                location.line()
            );
        }

        // Check if boot services are still active
        let boot_service_active = uefi::table::system_table_raw()
            .is_some_and(|system_table| !unsafe { system_table.as_ref() }.boot_services.is_null());

        // Stall for a significant amount of time to allow the user to read the message
        if boot_service_active {
            boot::stall(5_000_000);
        } else {
            let mut x = 0_u64;
            for i in 0..100_000_000 {
                unsafe {
                    core::ptr::write_volatile(&mut x, i);
                }
            }
        }
    }

    // Check if runtime services are available (they sould be)
    let runtime_services_available = uefi::table::system_table_raw()
        .is_some_and(|system_table| !unsafe { system_table.as_ref() }.runtime_services.is_null());

    // If possible, gracefully shutdown.
    // Otherwise, hang the system.
    if runtime_services_available {
        uefi::runtime::reset(uefi::runtime::ResetType::COLD, uefi::Status::ABORTED, None);
    } else {
        loop {
            x86_64::instructions::hlt();
        }
    }
}

#[entry]
fn efi_entry() -> Status {
    uefi::helpers::init().unwrap();

    // In debug mode, disable the watchdog timer
    #[cfg(debug_assertions)]
    let _ = boot::set_watchdog_timer(0, 0, None);

    bootloader::log::init_serial();

    debug!("BeskarOS bootloader is starting...");

    // Print basic firmware information and check for compatibility
    bootloader::system::check_firmware();

    bootloader::video::init();
    debug!("Video initialized");

    bootloader::log::init_screen();

    info!("BeskarOS bootloader started");

    #[cfg(debug_assertions)]
    debug!("Bootloader running in debug mode");

    bootloader::system::init();

    // Load Kernel file in RAM
    // Kernel is expected to be the only file named `kernelx64.elf` in the `efi` directory
    let kernel = {
        let file_content = bootloader::fs::load_file_from_efi_dir(cstr16!("kernelx64.elf"))
            .expect("Failed to load kernel");
        xmas_elf::ElfFile::new(file_content).expect("Failed to parse kernel")
    };
    info!("Kernel file loaded");

    let ramdisk = bootloader::fs::load_file_from_efi_dir(cstr16!("ramdisk.img"));
    if let Some(ramdisk) = ramdisk.as_ref() {
        info!("Ramdisk of size {}B loaded", ramdisk.len());
    }

    let mut memory_map = unsafe { boot::exit_boot_services(boot::MemoryType::LOADER_DATA) };
    debug!("Boot services exited");
    memory_map.sort();

    let (fralloc, mut pt, mut mappings) =
        bootloader::mem::init(memory_map, &kernel, ramdisk.as_deref());

    let boot_info = bootloader::create_boot_info(fralloc, &mut pt, &mut mappings);

    bootloader::log::log(format_args!("\n=== JUMPING TO KERNEL ===\n\n"));

    bootloader::video::clear_screen();

    unsafe {
        bootloader::chg_ctx(
            pt.kernel_level_4_frame.start_address().as_u64(),
            mappings.stack_top().as_u64(),
            mappings.entry_point().as_u64(),
            core::ptr::from_ref(boot_info) as u64,
        );
    };
}
