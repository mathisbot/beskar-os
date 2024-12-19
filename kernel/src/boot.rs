use core::sync::atomic::AtomicU8;

use crate::{
    cpu::{self, apic, interrupts},
    io, pci, process, screen, serdebug, serial, serinfo,
};
use bootloader::BootInfo;
use x86_64::PhysAddr;

use crate::{locals, logging, mem, time};

pub mod acpi;

/// Static reference to the kernel main function
///
/// This variable should be initialized by the BSP once the kernel is initialized.
/// It will be used by each core to start the kernel.
///
/// This function should never be called directly, but only by the `enter_kmain` function.
static mut KERNEL_MAIN: fn() -> ! = || loop {
    x86_64::instructions::hlt();
};

/// Static fence to ensure all cores enter `kmain` when they're all ready
static KERNEL_MAIN_FENCE: AtomicU8 = AtomicU8::new(0);

/// This function is the proper entry point called by the bootloader.
///
/// It should only be the entry for the BSP.
pub fn kbsp_entry(boot_info: &'static mut BootInfo, kernel_main: fn() -> !) -> ! {
    // Safety:
    // This line is only executed once on the BSP,
    // when no other core is currently running.
    unsafe { KERNEL_MAIN = kernel_main };

    let core_count = boot_info.cpu_count;

    bsp_init(boot_info);

    log::debug!("Core count: {}", core_count);

    apic::ap::start_up_aps(core_count);

    log::debug!("Kernel initialized");

    enter_kmain()
}

fn bsp_init(boot_info: &'static mut BootInfo) {
    let BootInfo {
        framebuffer,
        recursive_index,
        memory_regions,
        rsdp_paddr: rsdp_addr,
        ..
    } = boot_info;

    // FIXME: Serial log everything?
    serial::logging::init();

    cpu::init();

    serinfo!("BeskarOS kernel starting...");

    time::tsc::calibrate();

    mem::init(*recursive_index, memory_regions);
    serinfo!("Memory initialized");

    // TODO: Get framebuffer from PCI ?
    let screen_info = framebuffer.info().into();
    screen::init(framebuffer.buffer_mut(), screen_info);
    serdebug!(
        "Screen initialized with size: {}x{}",
        screen_info.width,
        screen_info.height
    );

    logging::init();
    serinfo!("Screen logging initialized");

    locals::init();

    locals!().gdt().init_load();

    process::init();

    interrupts::init();

    // If the bootloader provided an RSDP address, we can initialize ACPI.
    rsdp_addr.map(|rsdp_addr| acpi::init(PhysAddr::new(rsdp_addr)));
    time::hpet::init();

    apic::init_lapic();
    apic::init_ioapic();

    pci::init();

    io::init();
}

/// This function is called by each core once they're ready to start the kernel.
///
/// It will wait for all cores to be ready before starting the kernel,
/// i.e. entering `KERNEL_MAIN`.
pub(crate) fn enter_kmain() -> ! {
    KERNEL_MAIN_FENCE.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    while KERNEL_MAIN_FENCE.load(core::sync::atomic::Ordering::Acquire)
        != locals::get_ready_core_count()
    {
        core::hint::spin_loop();
    }

    unsafe { KERNEL_MAIN() }
}

#[macro_export]
/// Macro to define the kernel main function.
///
/// It should only be used once.
macro_rules! kernel_main {
    ($path:path) => {
        /// Entry of the kernel called by the bootloader.
        ///
        /// This should only be the entry point for the BSP.
        fn __bootloader_entry_point(boot_info: &'static mut bootloader::BootInfo) -> ! {
            $crate::boot::kbsp_entry(boot_info, $path);
        }
        bootloader::entry_point!(__bootloader_entry_point);
    };
}
