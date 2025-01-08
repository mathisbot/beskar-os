use x86_64::{PhysAddr, VirtAddr, structures::paging::PageTableFlags};

use crate::mem::page_alloc::pmap::PhysicalMapping;
use hyperdrive::locks::mcs::MUMcsLock;

static XHCI: MUMcsLock<Xhci> = MUMcsLock::uninit();

pub fn init(mut xhci_paddrs: impl Iterator<Item = PhysAddr>) {
    // TODO: Support multiple xHCI controllers
    let Some(first_xhci_paddr) = xhci_paddrs.next() else {
        crate::warn!("No xHCI controller found");
        return;
    };

    let xhci = Xhci::new(first_xhci_paddr);
    XHCI.init(xhci);

    XHCI.with_locked(|xhci| {
        crate::debug!("xHCI Capabilities Register: {:?}", xhci.cap);
    });
}

#[derive(Debug)]
/// See <https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/extensible-host-controler-interface-usb-xhci.pdf>
pub struct Xhci {
    cap: CapabilitiesRegister,
    _physical_mapping: PhysicalMapping,
}

impl Xhci {
    #[must_use]
    pub fn new(paddr: PhysAddr) -> Self {
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE
            | PageTableFlags::NO_CACHE;

        let physical_mapping = PhysicalMapping::new(paddr, 128, flags);
        let vaddr = physical_mapping.translate(paddr).unwrap();

        let cap = CapabilitiesRegister::new(vaddr);

        Self {
            cap,
            _physical_mapping: physical_mapping,
        }
    }
}

struct CapabilitiesRegister {
    /// Offset 0x00
    caplength: *mut u8,
    // Offset 0x02
    hci_version: *mut u16,
    // Offset 0x04
    hcs_params1: *mut u32,
    // Offset 0x08
    hcs_params2: *mut u32,
    // Offset 0x0C
    hcs_params3: *mut u32,
    // Offset 0x10
    hcc_params1: *mut u32,
    // Offset 0x14
    dboff: *mut u32,
    // Offset 0x18
    rtsoff: *mut u32,
    // Offset 0x1C
    hcc_params2: *mut u32,
}

impl core::fmt::Debug for CapabilitiesRegister {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CapabilitiesRegister")
            .field("caplength", unsafe { &*self.caplength })
            .field("hci_version", unsafe { &*self.hci_version })
            .field("hcs_params1", unsafe { &*self.hcs_params1 })
            .field("hcs_params2", unsafe { &*self.hcs_params2 })
            .field("hcs_params3", unsafe { &*self.hcs_params3 })
            .field("hcc_params1", unsafe { &*self.hcc_params1 })
            .field("dboff", unsafe { &*self.dboff })
            .field("rtsoff", unsafe { &*self.rtsoff })
            .field("hcc_params2", unsafe { &*self.hcc_params2 })
            .finish()
    }
}

impl CapabilitiesRegister {
    #[must_use]
    pub const fn new(base: VirtAddr) -> Self {
        let base_ptr = base.as_mut_ptr::<u32>();

        unsafe {
            let caplength = base_ptr.cast::<u8>();
            let hci_version = base_ptr.byte_add(0x02).cast::<u16>();
            let hcs_params1 = base_ptr.byte_add(0x04);
            let hcs_params2 = base_ptr.byte_add(0x08);
            let hcs_params3 = base_ptr.byte_add(0x0C);
            let hcc_params1 = base_ptr.byte_add(0x10);
            let dboff = base_ptr.byte_add(0x14);
            let rtsoff = base_ptr.byte_add(0x18);
            let hcc_params2 = base_ptr.byte_add(0x1C);

            // FIXME: This doesn't seem to stand when using the QEMU xHCI controller
            // assert_ne!(hci_version.read(), 0, "xHCI version is 0");

            Self {
                caplength,
                hci_version,
                hcs_params1,
                hcs_params2,
                hcs_params3,
                hcc_params1,
                dboff,
                rtsoff,
                hcc_params2,
            }
        }
    }
}

#[inline]
pub fn with_xhci<T, F: FnOnce(&mut Xhci) -> T>(f: F) -> Option<T> {
    XHCI.try_with_locked(f)
}
