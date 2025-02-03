pub mod acpi;
pub mod hpet;
pub mod nic;
pub mod pci;
pub mod storage;
pub mod tsc;
pub mod usb;

pub fn init() {
    if matches!(pci::init(), Err(_)) {
        panic!("No PCI devices found");
    }

    let _ = storage::init();
    let _ = usb::init();
    let _ = nic::init();
}
