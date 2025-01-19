use core::sync::atomic::AtomicBool;

use crate::drivers::{acpi, hpet};

static HPET_INIT: AtomicBool = AtomicBool::new(false);

pub fn init() {
    let hpet_table = acpi::ACPI.get().and_then(acpi::Acpi::hpet);
    if let Some(hpet) = hpet_table {
        hpet::init(hpet);
        HPET_INIT.store(true, core::sync::atomic::Ordering::Relaxed);
    }
}

pub(super) fn wait_ms(ms: u64) {
    let period_100ps =
        u64::from(hpet::with_hpet(|hpet| hpet.general_capabilities().period())) / 100_000;
    let start = hpet::with_hpet(|hpet| hpet.main_counter_value().get_value());

    let target = start + ms * 1_000_000 / period_100ps * 10;

    while hpet::main_counter_value() < target {
        core::hint::spin_loop();
    }
}

pub(super) fn is_init() -> bool {
    HPET_INIT.load(core::sync::atomic::Ordering::Relaxed)
}
