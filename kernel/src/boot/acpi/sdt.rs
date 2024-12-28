use core::mem::offset_of;

use x86_64::{structures::paging::PageTableFlags, PhysAddr, VirtAddr};

use super::AcpiRevision;
use crate::mem::page_alloc::pmap::PhysicalMapping;

pub mod fadt;
pub mod hpet_table;
pub mod madt;

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
/// System Descriptor Table (SDT) header.
pub struct SdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: [u8; 4],
    creator_revision: u32,
}

/// System Descriptor Table (SDT) trait.
///
/// ## Safety
///
/// The implementor must ensure that the memory is mapped and readable, as well as making sure
/// the underlying data is indeed a valid SDT.
pub unsafe trait Sdt {
    fn start(&self) -> *const u8;

    fn signature(&self) -> &[u8; 4] {
        unsafe {
            &*(self
                .start()
                .add(offset_of!(SdtHeader, signature))
                .cast::<[u8; 4]>())
        }
    }

    fn length(&self) -> u32 {
        unsafe {
            self.start()
                .add(offset_of!(SdtHeader, length))
                .cast::<u32>()
                .read_unaligned()
        }
    }

    fn revision(&self) -> u8 {
        unsafe {
            self.start()
                .add(offset_of!(SdtHeader, revision))
                .read_unaligned()
        }
    }

    fn oem_id(&self) -> &[u8; 6] {
        unsafe {
            &*(self
                .start()
                .add(offset_of!(SdtHeader, oem_id))
                .cast::<[u8; 6]>())
        }
    }

    fn oem_table_id(&self) -> &[u8; 8] {
        unsafe {
            &*(self
                .start()
                .add(offset_of!(SdtHeader, oem_table_id))
                .cast::<[u8; 8]>())
        }
    }

    fn oem_revision(&self) -> u32 {
        unsafe {
            self.start()
                .add(offset_of!(SdtHeader, oem_revision))
                .cast::<u32>()
                .read_unaligned()
        }
    }

    fn creator_id(&self) -> u32 {
        unsafe {
            self.start()
                .add(offset_of!(SdtHeader, creator_id))
                .cast::<u32>()
                .read_unaligned()
        }
    }

    fn creator_revision(&self) -> u32 {
        unsafe {
            self.start()
                .add(offset_of!(SdtHeader, creator_revision))
                .cast::<u32>()
                .read_unaligned()
        }
    }

    fn validate(&self) -> bool {
        let start_ptr = self.start();

        let mut sum = 0u8;
        for i in 0..usize::try_from(self.length()).unwrap() {
            sum = sum.wrapping_add(unsafe { start_ptr.add(i).read_unaligned() });
        }

        sum == 0
    }
}

#[derive(Debug)]
pub struct Rsdt {
    start_vaddr: VirtAddr,
    acpi_revision: AcpiRevision,
    _physical_mapping: PhysicalMapping,
}

// Safety:
// RSDT is a valid SDT.
unsafe impl Sdt for Rsdt {
    fn start(&self) -> *const u8 {
        self.start_vaddr.as_ptr()
    }
}

impl Rsdt {
    pub fn load(rsdt_paddr: PhysAddr) -> Self {
        let flags =
            PageTableFlags::PRESENT | x86_64::structures::paging::PageTableFlags::NO_EXECUTE;

        let physical_mapping = PhysicalMapping::new(rsdt_paddr, size_of::<SdtHeader>(), flags);
        let rsdt_vaddr = physical_mapping.translate(rsdt_paddr).unwrap();

        let rsdt = Self {
            start_vaddr: rsdt_vaddr,
            acpi_revision: super::ACPI_REVISION.load(),
            _physical_mapping: physical_mapping,
        };

        assert_eq!(rsdt.revision(), 1, "Unsupported RSDT revision");
        assert_eq!(
            rsdt.signature(),
            match rsdt.acpi_revision {
                AcpiRevision::V1 => b"RSDT",
                AcpiRevision::V2 => b"XSDT",
            },
            "Invalid RSDT signature"
        );

        let rsdt_length = usize::try_from(rsdt.length()).unwrap();

        drop(rsdt);

        let table_mapping = PhysicalMapping::new(rsdt_paddr, rsdt_length, flags);
        let rsdt_vaddr = table_mapping.translate(rsdt_paddr).unwrap();

        let rsdt = Self {
            start_vaddr: rsdt_vaddr,
            acpi_revision: super::ACPI_REVISION.load(),
            _physical_mapping: table_mapping,
        };

        // Validate checksum
        assert!(rsdt.validate(), "RSDT checksum failed");

        rsdt
    }

    #[must_use]
    #[inline]
    pub fn sdt_table_length(&self) -> usize {
        (usize::try_from(self.length()).unwrap() - size_of::<SdtHeader>())
            / match self.acpi_revision {
                AcpiRevision::V1 => size_of::<u32>(),
                AcpiRevision::V2 => size_of::<u64>(),
            }
    }

    #[must_use]
    /// Returns the physical address of the table with the given signature.
    ///
    /// The table is NOT validated.
    pub fn locate_table(&self, signature: Signature) -> Option<PhysAddr> {
        if self.acpi_revision != AcpiRevision::V2 {
            todo!("ACPI 1.0 RSDT table lookup is not implemented yet");
        }

        let start_ptr = unsafe {
            self.start_vaddr
                .as_ptr::<PhysAddr>()
                .byte_add(size_of::<SdtHeader>())
        };

        for i in 0..self.sdt_table_length() {
            let paddr = unsafe { start_ptr.add(i).read_unaligned() };

            let flags = x86_64::structures::paging::PageTableFlags::PRESENT
                | x86_64::structures::paging::PageTableFlags::NO_EXECUTE;

            let physical_mapping = PhysicalMapping::new(paddr, size_of::<SdtHeader>(), flags);
            let table_vaddr = physical_mapping.translate(paddr).unwrap();
            let header = unsafe { &*table_vaddr.as_ptr::<SdtHeader>() };

            if &header.signature == Into::<&[u8; 4]>::into(signature) {
                return Some(paddr);
            }
        }

        None
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Signature {
    Madt,
    Fadt,
    Hpet,
}

impl From<Signature> for &'static [u8; 4] {
    fn from(sig: Signature) -> Self {
        match sig {
            Signature::Madt => b"APIC",
            Signature::Fadt => b"FACP",
            Signature::Hpet => b"HPET",
        }
    }
}

/// Map a whole SDT.
///
/// ## Safety
///
/// The provided physical address must point to a potentially valid SDT.
unsafe fn map(phys_addr: PhysAddr) -> PhysicalMapping {
    let flags = PageTableFlags::PRESENT | x86_64::structures::paging::PageTableFlags::NO_EXECUTE;

    let header_mapping = PhysicalMapping::new(phys_addr, core::mem::size_of::<SdtHeader>(), flags);
    let header_vaddr = header_mapping.translate(phys_addr).unwrap();

    let table_length = unsafe {
        header_vaddr
            .as_ptr::<u32>()
            .byte_add(offset_of!(SdtHeader, length))
            .read_unaligned()
    };

    drop(header_mapping);

    PhysicalMapping::new(phys_addr, usize::try_from(table_length).unwrap(), flags)
}

#[macro_export]
/// A macro for implementing the basics of a SDT.
///
/// In particular, it provides a struct definition to handle the SDT in a proper way,
/// as well as a `load` function to create a validated instance of the struct.
macro_rules! impl_sdt {
    ($name:ident) => {
        #[derive(Debug)]
        pub struct $name {
            start_vaddr: x86_64::VirtAddr,
            _physical_mapping: $crate::mem::page_alloc::pmap::PhysicalMapping,
        }

        unsafe impl $crate::boot::acpi::sdt::Sdt for $name {
            fn start(&self) -> *const u8 {
                self.start_vaddr.as_ptr()
            }
        }

        impl $name {
            #[must_use]
            pub fn load(paddr: x86_64::PhysAddr) -> Self {
                use $crate::boot::acpi::sdt::Sdt;

                let mapping = unsafe { $crate::boot::acpi::sdt::map(paddr) };
                let vaddr = mapping.translate(paddr).unwrap();

                let table = Self {
                    start_vaddr: vaddr,
                    _physical_mapping: mapping,
                };

                assert!(table.validate(), concat!(stringify!($name), " is invalid"));

                table
            }
        }
    };
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
struct RawGenericAddress {
    address_space: u8,
    bit_width: u8,
    bit_offset: u8,
    access_size: u8,
    address: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct GenericAddress {
    address_space: AdressSpace,
    bit_width: u8,
    bit_offset: u8,
    access_size: AccessSize,
    address: u64,
}

impl From<RawGenericAddress> for GenericAddress {
    fn from(raw: RawGenericAddress) -> Self {
        Self {
            address_space: raw.address_space.into(),
            bit_width: raw.bit_width,
            bit_offset: raw.bit_offset,
            access_size: raw.access_size.try_into().expect("Invalid access size"),
            address: raw.address,
        }
    }
}

impl GenericAddress {
    #[must_use]
    #[inline]
    pub const fn address_space(&self) -> AdressSpace {
        self.address_space
    }

    #[must_use]
    #[inline]
    pub const fn bit_width(&self) -> u8 {
        self.bit_width
    }

    #[must_use]
    #[inline]
    pub const fn bit_offset(&self) -> u8 {
        self.bit_offset
    }

    #[must_use]
    #[inline]
    pub const fn access_size(&self) -> AccessSize {
        self.access_size
    }

    #[must_use]
    #[inline]
    pub const fn address(&self) -> u64 {
        self.address
    }

    #[must_use]
    #[inline]
    /// Adds an offset to the address.
    ///
    /// ## Safety
    ///
    /// The new address must be valid.
    pub const unsafe fn add(&self, offset: u64) -> Self {
        Self {
            address: self.address + offset,
            address_space: self.address_space,
            bit_width: self.bit_width,
            bit_offset: self.bit_offset,
            access_size: self.access_size,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum AdressSpace {
    SystemMemory = 0x00,
    SystemIO = 0x01,
    PciConfigSpace = 0x02,
    EmbeddedController = 0x03,
    SMBus = 0x04,
    SystemCMOS = 0x05,
    PciBarTarget = 0x06,
    Ipmi = 0x07,
    GeneralPurposeIO = 0x08,
    GenericSerialBus = 0x09,
    PlatformCommunicationChannel = 0x0A,
    Reserved,
    OemDefined,
}

impl From<u8> for AdressSpace {
    fn from(value: u8) -> Self {
        match value {
            0x00 => Self::SystemMemory,
            0x01 => Self::SystemIO,
            0x02 => Self::PciConfigSpace,
            0x03 => Self::EmbeddedController,
            0x04 => Self::SMBus,
            0x05 => Self::SystemCMOS,
            0x06 => Self::PciBarTarget,
            0x07 => Self::Ipmi,
            0x08 => Self::GeneralPurposeIO,
            0x09 => Self::GenericSerialBus,
            0x0A => Self::PlatformCommunicationChannel,
            x if (0x0B..=0x7F).contains(&x) => Self::Reserved,
            x if x >= 0x80 => Self::OemDefined,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessSize {
    Undefined = 0x00,
    Byte = 0x01,
    Word = 0x02,
    DWord = 0x03,
    QWord = 0x04,
}

impl TryFrom<u8> for AccessSize {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Undefined),
            1 => Ok(Self::Byte),
            2 => Ok(Self::Word),
            3 => Ok(Self::DWord),
            4 => Ok(Self::QWord),
            x => Err(x),
        }
    }
}
