pub mod acpi;
pub mod hpet;
pub mod nic;
pub mod pci;
pub mod storage;
pub mod usb;

pub fn init() {
    pci::init();

    storage::init();
    usb::init();
    nic::init();
}
