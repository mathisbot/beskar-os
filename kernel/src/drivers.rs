pub mod acpi;
pub mod hpet;
pub mod nic;
pub mod pci;
pub mod storage;
pub mod tsc;
pub mod usb;

pub fn init() {
    let pci_init_result = pci::init();
    assert!(pci_init_result.is_ok(), "No PCI devices found");

    let _ = storage::init();
    let _ = usb::init();
    let _ = nic::init();
}
