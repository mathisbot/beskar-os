use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{debug, info, warn};
use beskar_hal::registers::{Cr0, Efer};
use uefi::{proto::pi::mp::MpServices, system};

static CORE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    enable_and_count_cores();
    enable_cpu_features();
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
    debug!("Enabled cores: {}", proc_count.enabled);

    CORE_COUNT.store(proc_count.enabled, Ordering::Relaxed);
}

pub fn core_count() -> usize {
    CORE_COUNT.load(Ordering::Relaxed)
}

fn enable_cpu_features() {
    unsafe { Efer::insert_flags(Efer::NO_EXECUTE_ENABLE) };
    unsafe { Cr0::insert_flags(Cr0::WRITE_PROTECT) };
}
