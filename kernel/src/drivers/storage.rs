use crate::drivers::pci;
use alloc::vec::Vec;

use beskar_core::drivers::{DriverError, DriverResult};

pub mod ahci;
pub mod nvme;

pub fn init() -> DriverResult<()> {
    let mut ahci_controllers = Vec::new();
    let mut nvme = Vec::new();

    pci::with_pci_handler(|handler| {
        handler
            .devices()
            .iter()
            .filter(|device| device.csp().class() == pci::Class::MassStorage)
            .copied()
            .for_each(|d| {
                if d.csp().subclass() == 0x06 && d.csp().prog_if() == 0x01 {
                    ahci_controllers.push(d);
                } else if d.csp().subclass() == 0x08 && d.csp().prog_if() == 0x02 {
                    nvme.push(d);
                }
            });
    });

    let ahci_res = ahci::init(&ahci_controllers);
    let nvme_res = nvme::init(&nvme);

    if matches!(ahci_res, Err(_)) && matches!(nvme_res, Err(_)) {
        Err(DriverError::Absent)
    } else {
        Ok(())
    }
}
