use crate::boot::acpi;
use crate::cpu::hpet;

pub fn init() {
    let hpet_table = acpi::ACPI.get().map_or(None, |acpi| acpi.hpet());
    hpet_table.map(hpet::init);
}
