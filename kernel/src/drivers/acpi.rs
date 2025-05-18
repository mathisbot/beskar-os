use crate::mem::page_alloc::pmap::PhysicalMapping;
use ::acpi::Acpi;
use beskar_core::arch::{PhysAddr, paging::M4KiB};
use hyperdrive::once::Once;

pub static ACPI: Once<Acpi<PhysicalMapping<M4KiB>>> = Once::uninit();

pub fn init(rsdp_paddr: PhysAddr) {
    let acpi = Acpi::from_rsdp_paddr(rsdp_paddr);
    ACPI.call_once(|| acpi);
    video::debug!("ACPI initialized");
}
