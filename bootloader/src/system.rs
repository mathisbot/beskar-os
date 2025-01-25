use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{debug, info, warn};
use uefi::{proto::pi::mp::MpServices, system};
use x86_64::registers::control::{Cr0, Cr0Flags, Efer, EferFlags};

pub mod acpi;

static CORE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    enable_and_count_cores();
    enable_cpu_features();
    // Find the hopefully available XSDP/RSDP
    acpi::init();
}

/// Print firmware information and check for compatibility.
pub fn check_firmware() {
    info!("Firmware Vendor: {}", system::firmware_vendor());
    info!("Firmware Revision: {}", system::firmware_revision());

    let rev = system::uefi_revision();

    info!("UEFI specification: v{}.{}", rev.major(), rev.minor() / 10);

    assert_eq!(rev.major(), 2, "Running on an unsupported version of UEFI");
    if rev.minor() < 30 {
        warn!("Old version of UEFI 2, some features might not be available.");
    }
}

/// Enable all available healthy cores and return the number of enabled cores.
fn enable_and_count_cores() {
    let mps = {
        let mp_handle = uefi::boot::get_handle_for_protocol::<MpServices>().unwrap();
        uefi::boot::open_protocol_exclusive::<MpServices>(mp_handle).unwrap()
    };

    debug!("Enabling all cores...");
    for i in 0..mps.get_number_of_processors().unwrap().total {
        if i != mps.who_am_i().unwrap() {
            let info = mps.get_processor_info(i).unwrap();
            if info.is_healthy() {
                mps.enable_disable_ap(i, true, Some(true)).unwrap();
            } else {
                warn!("Processor {} is not healthy, skipping it.", i);
                // Make sure it is disabled
                mps.enable_disable_ap(i, false, Some(false)).unwrap();
            }
        }
    }

    let proc_count = mps.get_number_of_processors().unwrap();
    if proc_count.enabled != proc_count.total {
        warn!(
            "Only {} out of {} processors could be enabled",
            proc_count.enabled, proc_count.total
        );
    }

    CORE_COUNT.store(proc_count.enabled, Ordering::Relaxed);
}

pub fn core_count() -> usize {
    CORE_COUNT.load(Ordering::Relaxed)
}

fn enable_cpu_features() {
    // Enable support for no execute pages.
    unsafe {
        Efer::update(|efer| {
            efer.insert(EferFlags::NO_EXECUTE_ENABLE);
        });
    };

    // Enable support for write protection in Ring-0.
    unsafe {
        Cr0::update(|cr0| {
            cr0.insert(Cr0Flags::WRITE_PROTECT);
        });
    };
}
