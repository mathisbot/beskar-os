pub mod acpi;
pub mod hpet;
pub mod network;
pub mod pci;
pub mod usb;

pub fn init() {
    pci::init();

    usb::init();
    network::init();
}
