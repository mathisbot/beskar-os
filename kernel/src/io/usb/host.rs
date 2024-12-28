use x86_64::{structures::paging::PageTableFlags, PhysAddr, VirtAddr};

use crate::{mem::page_alloc::pmap::PhysicalMapping, utils::locks::MUMcsLock};

static XHCI: MUMcsLock<Xhci> = MUMcsLock::uninit();

pub fn init(paddr: PhysAddr) {
    let xhci = Xhci::new(paddr);
    XHCI.init(xhci);

    XHCI.with_locked(|xhci| {
        log::debug!("xHCI Capabilities Register: {:?}", xhci.cap);
    });
}

#[derive(Debug)]
pub struct Xhci<'a> {
    cap: CapabilitiesRegister<'a>,
    _physical_mapping: PhysicalMapping,
}

impl Xhci<'_> {
    #[must_use]
    pub fn new(paddr: PhysAddr) -> Self {
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE
            | PageTableFlags::NO_CACHE;

        let physical_mapping = PhysicalMapping::new(paddr, 32, flags);
        let vaddr = physical_mapping.translate(paddr).unwrap();

        let cap = CapabilitiesRegister::new(vaddr);

        Self {
            cap,
            _physical_mapping: physical_mapping,
        }
    }
}

#[derive(Debug)]
struct CapabilitiesRegister<'a> {
    /// Offset 0x00
    caplength: &'a mut u8,
    // Offset 0x02
    hci_version: &'a mut u16,
    // Offset 0x04
    hcs_params1: &'a mut u32,
    // Offset 0x08
    hcs_params2: &'a mut u32,
    // Offset 0x0C
    hcs_params3: &'a mut u32,
    // Offset 0x10
    hcc_params1: &'a mut u32,
    // Offset 0x14
    dboff: &'a mut u32,
    // Offset 0x18
    rtsoff: &'a mut u32,
    // Offset 0x1C
    hcc_params2: &'a mut u32,
}

impl CapabilitiesRegister<'_> {
    #[must_use]
    pub const fn new(base: VirtAddr) -> Self {
        let base_ptr = base.as_mut_ptr::<u32>();

        unsafe {
            let caplength = &mut *base_ptr.cast::<u8>();
            let hci_version = &mut *base_ptr.byte_add(0x02).cast::<u16>();
            let hcs_params1 = &mut *base_ptr.byte_add(0x04);
            let hcs_params2 = &mut *base_ptr.byte_add(0x08);
            let hcs_params3 = &mut *base_ptr.byte_add(0x0C);
            let hcc_params1 = &mut *base_ptr.byte_add(0x10);
            let dboff = &mut *base_ptr.byte_add(0x14);
            let rtsoff = &mut *base_ptr.byte_add(0x18);
            let hcc_params2 = &mut *base_ptr.byte_add(0x1C);

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
