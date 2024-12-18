use crate::boot::acpi;
use crate::cpu::hpet;

pub fn init() {
    let hpet_table = acpi::ACPI.get().and_then(acpi::Acpi::hpet);
    hpet_table.map(hpet::init);
}
