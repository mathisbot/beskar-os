use core::sync::atomic::AtomicUsize;

use crate::{
    arch::{self, ap, apic, interrupts},
    drivers, locals, mem, process, screen, time,
};
use bootloader::BootInfo;

/// Static reference to the kernel main function
///
/// This variable should be initialized by the BSP once the kernel is initialized.
/// It will be used by each core to start the kernel.
///
/// This function should never be called directly, but only by the `enter_kmain` function.
static mut KERNEL_MAIN: fn() -> ! = || loop {
    crate::arch::halt();
};

/// Static fence to ensure all cores enter `kmain` when they're all ready
static KERNEL_MAIN_FENCE: AtomicUsize = AtomicUsize::new(0);

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

    crate::debug!("Starting up APs. Core count: {}", core_count);

    ap::start_up_aps(core_count);

    enter_kmain()
}

fn bsp_init(boot_info: &'static mut BootInfo) {
    let BootInfo {
        framebuffer,
        recursive_index,
        memory_regions,
        rsdp_paddr,
        kernel_vaddr,
        ..
    } = boot_info;

    crate::log::init_serial();
    crate::debug!("Booting on BSP");

    // TODO: Get framebuffer from PCI ?
    screen::init(framebuffer);
    crate::log::init_screen();

    arch::init();

    crate::info!("BeskarOS kernel starting...");

    time::tsc::calibrate();

    mem::init(*recursive_index, memory_regions, *kernel_vaddr);
    crate::info!("Memory initialized");

    locals::init();

    locals!().gdt().init_load();

    process::init();

    interrupts::init();

    // If the bootloader provided an RSDP address, we can initialize ACPI.
    rsdp_paddr.map(drivers::acpi::init);
    time::hpet::init();

    apic::init_lapic();
    process::scheduler::set_scheduling(true);
    apic::init_ioapic();

    drivers::init();
}

/// Rust entry point for APs
///
/// This function is called by the AP trampoline code.
pub extern "C" fn kap_entry() -> ! {
    // Safety:
    // Values are coming from the BSP, so they are safe to use.
    unsafe {
        ap::load_ap_regs();
    }

    // Tell the BSP we are out of the trampoline spin lock,
    // allowing others to get their stack
    locals::core_jumped();

    ap_init();

    crate::debug!("AP {} started", locals!().core_id());

    crate::boot::enter_kmain()
}

fn ap_init() {
    arch::init();

    locals::init();

    locals!().gdt().init_load();

    process::init();

    arch::interrupts::init();

    arch::apic::init_lapic();
    process::scheduler::set_scheduling(true);
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
        ::bootloader::entry_point!(__bootloader_entry_point);
    };
}
