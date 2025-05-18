use crate::{
    arch::{self, apic, interrupts},
    drivers, locals, mem, process, storage, syscall, time,
};
use bootloader_api::{BootInfo, RamdiskInfo};
use core::sync::atomic::AtomicUsize;
use hyperdrive::once::Once;

/// Static reference to the kernel main function
///
/// This variable should be initialized by the BSP once the kernel is initialized.
/// It will be used by each core to start the kernel.
///
/// This function should never be called directly, but only by the `enter_kmain` function.
static KERNEL_MAIN: Once<fn() -> !> = Once::uninit();

/// Static fence to ensure all cores enter `kmain` when they're all ready
static KERNEL_MAIN_FENCE: AtomicUsize = AtomicUsize::new(0);

static RAMDISK: Once<Option<RamdiskInfo>> = Once::uninit();

/// This function is the proper entry point called by the bootloader.
///
/// It should only be the entry for the BSP.
pub fn kbsp_entry(boot_info: &'static mut BootInfo, kernel_main: fn() -> !) -> ! {
    KERNEL_MAIN.call_once(|| kernel_main);
    RAMDISK.call_once(|| boot_info.ramdisk_info);

    let core_count = boot_info.cpu_count;

    bsp_init(boot_info);

    video::debug!("Starting up APs. Core count: {}", core_count);

    arch::ap::start_up_aps(core_count);

    enter_kmain()
}

fn bsp_init(boot_info: &'static mut BootInfo) {
    let BootInfo {
        framebuffer,
        recursive_index,
        memory_regions,
        rsdp_paddr,
        kernel_info,
        ..
    } = boot_info;

    video::log::init_serial();
    video::debug!("Booting on BSP");

    video::screen::init(framebuffer);
    video::log::init_screen();

    arch::init();

    video::info!("BeskarOS kernel starting...");

    mem::init(*recursive_index, memory_regions, kernel_info);
    video::info!("Memory initialized");

    locals::init();

    // Safety: `locals!` provide a `'static` reference to the core locals.
    locals!()
        .gdt()
        .with_locked(|gdt| unsafe { gdt.init_load() });

    process::init();
    video::info!("Process subsystem initialized");

    // If the bootloader provided an RSDP address, we can initialize ACPI.
    rsdp_paddr.map(drivers::acpi::init);

    time::init();
    video::info!("Time subsystem initialized");

    interrupts::init();
    video::info!("Interrupts initialized");

    syscall::init();

    // TODO: Move into an architecture agnostic module
    apic::init_lapic();
    apic::init_ioapic();

    storage::init();
    video::info!("Storage subsystem initialized");
}

/// Rust entry point for APs
///
/// This function is called by the AP trampoline code.
pub extern "C" fn kap_entry() -> ! {
    // Safety:
    // Values are coming from the BSP, so they are safe to use.
    unsafe {
        arch::ap::load_ap_regs();
    }

    ap_init();

    video::debug!("AP {} started", locals!().core_id());

    crate::boot::enter_kmain()
}

fn ap_init() {
    arch::init();

    locals::init();

    // Safety: `locals!` provide a `'static` reference to the core locals.
    locals!()
        .gdt()
        .with_locked(|gdt| unsafe { gdt.init_load() });

    process::init();

    interrupts::init();

    syscall::init();

    arch::apic::init_lapic();
}

/// Returns the ramdisk data as readonly.
///
/// # Panics
///
/// Panics if the ramdisk is not initialized.
pub fn ramdisk() -> Option<&'static [u8]> {
    RAMDISK.get().unwrap().map(|rd| unsafe {
        core::slice::from_raw_parts(rd.vaddr().as_ptr(), rd.size().try_into().unwrap())
    })
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

    (KERNEL_MAIN.get().unwrap())()
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
        fn __bootloader_entry_point(boot_info: &'static mut ::bootloader_api::BootInfo) -> ! {
            $crate::boot::kbsp_entry(boot_info, $path);
        }
        ::bootloader_api::entry_point!(__bootloader_entry_point);
    };
}
