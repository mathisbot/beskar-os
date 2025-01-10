//! General PCI handling module.

mod commons;
pub use commons::{Bar, Class, Device};
mod express;
mod legacy;

pub fn init() {
    express::init();

    if !express::pcie_available() {
        crate::info!("PCIe not available, falling back to legacy PCI");
        legacy::init();
    }
}

pub trait PciHandler {
    #[must_use]
    /// Returns the list of devices found by the PCI handler.
    fn devices(&self) -> &[commons::Device];

    #[must_use]
    /// Read the raw value from the PCI configuration space
    ///
    /// Bar number must be 0 to 5 (inclusive).
    fn read_bar(&mut self, device: &commons::Device, bar: u8) -> Option<commons::Bar>;
}

pub fn with_pci_handler<T, F: FnOnce(&mut dyn PciHandler) -> T>(f: F) -> T {
    if express::pcie_available() {
        // Safety: PCIe is available, thus the handler is initialized.
        unsafe { express::with_pcie_handler(|h| f(h)).unwrap_unchecked() }
    } else {
        legacy::with_legacy_pci_handler(|h| f(h))
    }
}
