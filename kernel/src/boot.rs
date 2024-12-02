use core::sync::atomic::AtomicU8;

use crate::{
    acpi,
    cpu::{apic, interrupts},
    pci, screen, serdebug, serial, serinfo,
};
use bootloader::BootInfo;
use x86_64::PhysAddr;

use crate::{locals, logging, mem, time};

/// Static reference to the kernel main function
///
/// This variable should be initialized by the BSP once the kernel is initialized.
/// It will be used by each core to start the kernel.
static mut KERNEL_MAIN: fn() -> ! = || loop {
    x86_64::instructions::hlt();
};

/// Static fence to ensure all cores enter `kmain` when they're all ready
static KERNEL_MAIN_FENCE: AtomicU8 = AtomicU8::new(0);

/// This function is the proper entry point called by the bootloader.
/// It should only be the entry for the BSP.
pub fn kbsp_entry(boot_info: &'static mut BootInfo, kernel_main: fn() -> !) -> ! {
    // Safety:
    // This line is only executed on the BSP,
    // and no other core is currently running.
    unsafe { KERNEL_MAIN = kernel_main };

    let core_count = boot_info.cpu_count;

    bsp_init(boot_info);

    log::debug!("Core count: {}", core_count);

    apic::ap::start_up_aps(core_count);

    log::debug!("Kernel initialized");

    enter_kmain()
}

pub fn bsp_init(boot_info: &'static mut BootInfo) {
    let BootInfo {
        framebuffer,
        recursive_index,
        memory_regions,
        rsdp_paddr: rsdp_addr,
        ..
    } = boot_info;

    // Init one-time stuff

    crate::cpu::check_cpuid();

    // FIXME: Serial log everything?
    serial::init();

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

    interrupts::init();

    // If the bootloader provided an RSDP address, we can initialize ACPI.
    rsdp_addr.map(|rsdp_addr| acpi::init(PhysAddr::new(rsdp_addr)));

    apic::init();

    pci::init();
    // TODO: PCI ?
}

pub fn ap_init() {
    crate::cpu::check_cpuid();

    locals::init();

    locals!().gdt().init_load();

    interrupts::init();

    apic::init();
}

pub fn enter_kmain() -> ! {
    KERNEL_MAIN_FENCE.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    while KERNEL_MAIN_FENCE.load(core::sync::atomic::Ordering::SeqCst)
        != locals::get_ready_core_count()
    {
        core::hint::spin_loop();
    }

    unsafe { KERNEL_MAIN() }
}

#[macro_export]
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
