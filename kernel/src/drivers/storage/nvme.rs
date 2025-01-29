//! NVM Express Controller driver, according to
//! <https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-Revision-2.1-2024.08.05-Ratified.pdf>
//! (NVM Express Base Specification Revision 2.1)

use core::ptr::NonNull;

use beskar_core::arch::commons::{
    VirtAddr,
    paging::{Flags, M4KiB},
};
use hyperdrive::{
    locks::mcs::MUMcsLock,
    volatile::{ReadOnly, Volatile, WriteOnly},
};

use crate::{
    drivers::{
        DriverError, DriverResult,
        pci::{self, Bar, Device, msix::MsiX},
    },
    mem::page_alloc::pmap::PhysicalMapping,
};

static NVME_CONTROLLER: MUMcsLock<NvmeControllers> = MUMcsLock::uninit();

pub fn init(nvme: &[Device]) -> DriverResult<()> {
    if nvme.len() > 1 {
        crate::warn!("Multiple NVMe controllers found, using the first one");
    }
    let Some(nvme) = nvme.first() else {
        return Err(DriverError::Absent);
    };

    let controller = NvmeControllers::new(nvme);
    controller.init();

    crate::debug!(
        "NVMe controller initialized with version {}",
        controller.version()
    );

    NVME_CONTROLLER.init(controller);

    Ok(())
}

pub struct NvmeControllers {
    registers_base: VirtAddr,
    _pmap: PhysicalMapping,
}

impl NvmeControllers {
    // TODO: Custom error type
    pub fn new(dev: &Device) -> Self {
        let (Some(Bar::Memory(bar)), Some(msix)) =
            pci::with_pci_handler(|handler| (handler.read_bar(&dev, 0), MsiX::new(handler, &dev)))
        else {
            unreachable!("NVMe controller either have no memory BAR or no MSI-X capability");
        };

        let paddr = bar.base_address();

        let flags = Flags::MMIO_SUITABLE;
        let physical_mapping = PhysicalMapping::<M4KiB>::new(paddr, 0x38, flags);
        let registers_base = physical_mapping.translate(paddr).unwrap();

        Self {
            registers_base,
            _pmap: physical_mapping,
        }
    }

    pub fn init(&self) {
        // TODO: Initialize the controller
    }

    #[must_use]
    #[inline]
    pub const fn capabilities(&self) -> Capabilities {
        let ptr = NonNull::new(self.registers_base.as_mut_ptr()).unwrap();
        Capabilities(Volatile::new(ptr))
    }

    #[must_use]
    #[inline]
    pub const fn version(&self) -> Version {
        unsafe {
            self.registers_base
                .as_mut_ptr::<Version>()
                .byte_add(0x08)
                .read()
        }
    }

    #[must_use]
    #[inline]
    /// When using MSI-X, the interrupt mask table defined as part of MSI-X should be used to
    /// mask interrupts. Host software shall not access this property when configured for MSI-X.
    pub const fn intms(&self) -> Volatile<WriteOnly, u32> {
        let ptr = unsafe { self.registers_base.as_mut_ptr::<u32>().byte_add(0x0C) };
        Volatile::new(NonNull::new(ptr).unwrap())
    }

    #[must_use]
    #[inline]
    /// When using MSI-X, the interrupt mask table defined as part of MSI-X should be used to
    /// unmask interrupts. Host software shall not access this property when configured for MSI-X.
    pub const fn intmc(&self) -> Volatile<WriteOnly, u32> {
        let ptr = unsafe { self.registers_base.as_mut_ptr::<u32>().byte_add(0x10) };
        Volatile::new(NonNull::new(ptr).unwrap())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Version {
    tertiary: u8,
    minor: u8,
    major: u16,
}

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NVMe {}.{}.{}", self.major, self.minor, self.tertiary)
    }
}

pub struct Capabilities(Volatile<ReadOnly, u64>);

impl Capabilities {
    #[must_use]
    #[inline]
    fn read(&self) -> u64 {
        unsafe { self.0.read() }
    }

    #[must_use]
    #[inline]
    /// Maximum Queue Entries Supported
    pub fn mqes(&self) -> u16 {
        u16::try_from(self.read() & 0xFFFF).unwrap()
    }

    #[must_use]
    #[inline]
    /// Contiguous Queues Required
    pub fn cqr(&self) -> bool {
        (self.read() & (1 << 16)) != 0
    }

    #[must_use]
    #[inline]
    /// Arbitration Mechanism Support
    ///
    /// Bit 0: Weighted Round Robin with Urgent Priority Class
    /// Bit 1: Vendor Specific
    /// Bits 2-7: Always 0
    pub fn ams(&self) -> u8 {
        u8::try_from((self.read() & (1 << 17)) & 0b11).unwrap()
    }

    #[must_use]
    #[inline]
    /// Worst case time for the controller to be ready
    ///
    /// This field is in 500ms units, for a maximum value of 127.5 seconds.
    pub fn to(&self) -> u8 {
        u8::try_from((self.read() >> 24) & 0xFF).unwrap()
    }

    #[must_use]
    #[inline]
    /// Doorbell stride
    ///
    /// Each Submission Queue and Completion Queue Doorbell register is 32-bits in size.
    /// This register indicates the stride between doorbell registers.
    pub fn dstrd(&self) -> usize {
        let power = (self.read() >> 32) & 0xF;
        1 << (power + 2)
    }

    #[must_use]
    #[inline]
    /// NVM Subsystem Reset Support
    pub fn nssrs(&self) -> bool {
        (self.read() & (1 << 36)) != 0
    }

    #[must_use]
    #[inline]
    /// Command Sets Supported
    ///
    /// Bit 0: NVM command set
    /// Bits 1-5: Reserved
    /// Bit 6: IO command set
    /// Bit 7: No IO command set
    pub fn css(&self) -> u8 {
        u8::try_from((self.read() >> 37) & 0xF).unwrap()
    }

    #[must_use]
    #[inline]
    /// Boot Partition Support
    pub fn bps(&self) -> bool {
        (self.read() & (1 << 45)) != 0
    }

    #[must_use]
    #[inline]
    /// Controller Power Scope
    ///
    /// 0b00: Unknown
    /// 0b01: Controller scope
    /// 0b10: Domain scope
    /// 0b11: NVM subsystem scope
    pub fn cps(&self) -> u8 {
        u8::try_from((self.read() & (1 << 46)) & 0b11).unwrap()
    }

    #[must_use]
    #[inline]
    /// Minimum host memory page size that the controller supports.
    pub fn mpsmin(&self) -> u32 {
        let power = (self.read() >> 48) & 0xF;
        1 << (power + 12)
    }

    #[must_use]
    #[inline]
    /// Maximum host memory page size that the controller supports.
    pub fn mpsmax(&self) -> u32 {
        let power = (self.read() >> 52) & 0xF;
        1 << (power + 12)
    }
}
