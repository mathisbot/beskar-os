//! General PCI handling module.

mod commons;
pub use commons::{Bar, Class, Device};
mod express;
mod legacy;

pub fn init() {
    express::init();

    if !express::pcie_available() {
        legacy::init();
    }
}

pub trait PciHandler {
    fn devices(&self) -> &[commons::Device];
    fn read_bar(&mut self, device: &commons::Device, bar: u8) -> Option<commons::Bar>;
}

/// This function allows to use the current PCI handler.
///
/// `PciHandler` has a small API, and you can easily use a `match` statement
/// to further use methods from the current handler.
pub fn with_pci_handler<T, F: FnOnce(&mut dyn PciHandler) -> T>(f: F) -> T {
    if express::pcie_available() {
        // Safety: PCIe is available, thus the handler is initialized.
        unsafe { express::with_pcie_handler(|h| f(h)).unwrap_unchecked() }
    } else {
        legacy::with_legacy_pci_handler(|h| f(h))
    }
}
