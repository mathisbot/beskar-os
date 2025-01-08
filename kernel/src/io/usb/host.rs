// TODO: Remove
#![allow(dead_code)]

use core::ptr::NonNull;

use x86_64::{PhysAddr, VirtAddr, structures::paging::PageTableFlags};

use crate::mem::page_alloc::pmap::PhysicalMapping;
use hyperdrive::{
    locks::mcs::MUMcsLock,
    volatile::{Access, Volatile},
};

static XHCI: MUMcsLock<Xhci> = MUMcsLock::uninit();

pub fn init(mut xhci_paddrs: impl Iterator<Item = PhysAddr>) {
    // TODO: Support multiple xHCI controllers
    let Some(first_xhci_paddr) = xhci_paddrs.next() else {
        crate::warn!("No xHCI controller found");
        return;
    };

    let xhci = Xhci::new(first_xhci_paddr);
    XHCI.init(xhci);
}

#[derive(Debug)]
/// See <https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/extensible-host-controler-interface-usb-xhci.pdf>
pub struct Xhci {
    cap: CapabilitiesRegister,
    op1: OperationalRegister,
    _physical_mapping: PhysicalMapping,
}

impl Xhci {
    #[must_use]
    pub fn new(paddr: PhysAddr) -> Self {
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE
            | PageTableFlags::NO_CACHE;

        // At first, we only map enough memory to read the capabilities register
        let physical_mapping = PhysicalMapping::new(paddr, CapabilitiesRegister::MIN_LENGTH, flags);
        let vaddr = physical_mapping.translate(paddr).unwrap();

        let cap = CapabilitiesRegister::new(vaddr);
        let cap_length = usize::from(cap.cap_length());

        // We can now map more memory to read the operational registers
        let physical_mapping =
            PhysicalMapping::new(paddr, cap_length + OperationalRegister::LENGTH, flags);

        let op1 = OperationalRegister::new(vaddr + u64::try_from(cap_length).unwrap());

        Self {
            cap,
            op1,
            _physical_mapping: physical_mapping,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CapabilitiesRegister {
    base: Volatile<u32>,
}

impl CapabilitiesRegister {
    pub const MIN_LENGTH: usize = 0x20;

    const CAP_LENGTH: usize = 0x00;
    const HCI_VERSION: usize = 0x02;
    const HCS_PARAMS1: usize = 0x04;
    const HCS_PARAMS2: usize = 0x08;
    const HCS_PARAMS3: usize = 0x0C;
    const HCC_PARAMS1: usize = 0x10;
    const DBOFF: usize = 0x14;
    const RTSOFF: usize = 0x18;
    const HCC_PARAMS2: usize = 0x1C;

    #[must_use]
    pub const fn new(base: VirtAddr) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap(), Access::ReadOnly);
        Self { base }
    }

    #[must_use]
    /// Offset of the first operational register from the base address
    pub fn cap_length(&self) -> u8 {
        unsafe { self.base.cast::<u8>().add(Self::CAP_LENGTH).read() }
    }

    #[must_use]
    pub fn hci_version(&self) -> u16 {
        unsafe { self.base.cast::<u16>().byte_add(Self::HCI_VERSION).read() }
    }

    #[must_use]
    pub fn hcs_params1(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCS_PARAMS1).read() }
    }

    #[must_use]
    pub fn hcs_params2(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCS_PARAMS2).read() }
    }

    #[must_use]
    pub fn hcs_params3(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCS_PARAMS3).read() }
    }

    #[must_use]
    pub fn hcc_params1(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCC_PARAMS1).read() }
    }

    #[must_use]
    /// Doorbell array offset
    pub fn dboff(&self) -> u32 {
        unsafe { self.base.byte_add(Self::DBOFF).read() }
    }

    #[must_use]
    /// Runtime register space offset
    pub fn rtsoff(&self) -> u32 {
        unsafe { self.base.byte_add(Self::RTSOFF).read() }
    }

    #[must_use]
    pub fn hcc_params2(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCC_PARAMS2).read() }
    }
}

#[derive(Debug)]
struct OperationalRegister {
    base: Volatile<u32>,
}

impl OperationalRegister {
    pub const LENGTH: usize = 0x3C;

    const COMMAND: usize = 0x00;
    const STATUS: usize = 0x04;
    const PAGE_SIZE: usize = 0x08;
    const DEV_NOTIFICATION: usize = 0x14;
    const CMD_RING: usize = 0x18;
    const DCBAAP: usize = 0x30;
    const CONFIGURE: usize = 0x38;

    #[must_use]
    pub const fn new(base: VirtAddr) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap(), Access::ReadWrite);
        Self { base }
    }

    #[must_use]
    pub fn command(&self) -> Volatile<u32> {
        unsafe { self.base.byte_add(Self::COMMAND) }
    }

    #[must_use]
    pub fn status(&self) -> Volatile<u32> {
        unsafe { self.base.byte_add(Self::STATUS) }
    }

    #[must_use]
    /// If bit `i` is set, the controller supports a page size of 2^(12 + i) bytes.
    pub fn page_size(&self) -> Volatile<u32> {
        unsafe { self.base.byte_add(Self::PAGE_SIZE) }
    }

    #[must_use]
    pub fn dev_notification(&self) -> Volatile<u32> {
        unsafe { self.base.byte_add(Self::DEV_NOTIFICATION) }
    }

    #[must_use]
    /// Reading the command ring registor (or bits of it) provides '0'.
    pub fn cmd_ring(&self) -> Volatile<u64> {
        unsafe { self.base.cast::<u64>().byte_add(Self::CMD_RING) }
    }

    #[must_use]
    pub fn dcbaap(&self) -> Volatile<u64> {
        unsafe { self.base.cast::<u64>().byte_add(Self::DCBAAP) }
    }

    #[must_use]
    pub fn configure(&self) -> Volatile<u32> {
        unsafe { self.base.byte_add(Self::CONFIGURE) }
    }
}

#[inline]
pub fn with_xhci<T, F: FnOnce(&mut Xhci) -> T>(f: F) -> Option<T> {
    XHCI.try_with_locked(f)
}
