//! PCI common definitions

use x86_64::PhysAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Device Class, Subclass, Programming Interface
pub struct Csp {
    class: Class,
    subclass: u8,
    prog_if: u8,
}

impl Csp {
    #[must_use]
    #[inline]
    pub const fn new(class: Class, subclass: u8, prog_if: u8) -> Self {
        Self {
            class,
            subclass,
            prog_if,
        }
    }

    #[must_use]
    #[inline]
    pub const fn class(self) -> Class {
        self.class
    }

    #[must_use]
    #[inline]
    pub const fn subclass(self) -> u8 {
        self.subclass
    }

    #[must_use]
    #[inline]
    pub const fn prog_if(self) -> u8 {
        self.prog_if
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    MassStorage = 0x01,
    Network = 0x02,
    Display = 0x03,
    Multimedia = 0x04,
    Memory = 0x05,
    Bridge = 0x06,
    SimpleCommunication = 0x07,
    BaseSystemPeripherals = 0x08,
    InputDevice = 0x09,
    DockingStation = 0x0A,
    Processor = 0x0B,
    SerialBus = 0x0C,
    Wireless = 0x0D,
    IntelligentIo = 0x0E,
    SatelliteCommunication = 0x0F,
    Encryption = 0x10,
    SignalProcessing = 0x11,
    ProcessingAccelerator = 0x12,
    NonEssentialInstrumentation = 0x13,
    CoProcessor = 0x40,
    Unassigned = 0xFF,
    Unknown,
}

impl From<u8> for Class {
    fn from(value: u8) -> Self {
        match value {
            0x01 => Self::MassStorage,
            0x02 => Self::Network,
            0x03 => Self::Display,
            0x04 => Self::Multimedia,
            0x05 => Self::Memory,
            0x06 => Self::Bridge,
            0x07 => Self::SimpleCommunication,
            0x08 => Self::BaseSystemPeripherals,
            0x09 => Self::InputDevice,
            0x0A => Self::DockingStation,
            0x0B => Self::Processor,
            0x0C => Self::SerialBus,
            0x0D => Self::Wireless,
            0x0E => Self::IntelligentIo,
            0x0F => Self::SatelliteCommunication,
            0x10 => Self::Encryption,
            0x11 => Self::SignalProcessing,
            0x12 => Self::ProcessingAccelerator,
            0x13 => Self::NonEssentialInstrumentation,
            0x40 => Self::CoProcessor,
            0xFF => Self::Unassigned,
            _ => Self::Unknown,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RegisterOffset {
    VendorId = 0x00,
    DeviceId = 0x02,
    Command = 0x04,
    Status = 0x06,
    RevisionId = 0x08,
    ProgIf = 0x09,
    Subclass = 0x0A,
    Class = 0x0B,
    CacheLineSize = 0x0C,
    LatencyTimer = 0x0D,
    HeaderType = 0x0E,
    Bist = 0x0F,
    Bar0 = 0x10,
    Bar1 = 0x14,
    Bar2 = 0x18,
    Bar3 = 0x1C,
    Bar4 = 0x20,
    Bar5 = 0x24,
    CardbusCisPointer = 0x28,
    SubsystemId = 0x2C,
    SubsystemVendorId = 0x2E,
    ExpansionRomBaseAddress = 0x30,
    CapabilitiesPointer = 0x34,
    InterruptLine = 0x3C,
    InterruptPin = 0x3D,
    MinGrant = 0x3E,
    MaxLatency = 0x3F,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Bus, Device, Function address
pub struct BdfAddress {
    /// 0-255
    bus: u8,
    /// 0-31
    device: u8,
    /// 0-7
    function: u8,
}

impl BdfAddress {
    #[must_use]
    #[inline]
    pub const fn new(bus: u8, device: u8, function: u8) -> Self {
        assert!(device <= 0b1_1111, "Device number must be less than 32");
        assert!(function <= 0b111, "Function number must be less than 8");
        Self {
            bus,
            device,
            function,
        }
    }

    #[must_use]
    #[inline]
    pub const fn bus(self) -> u8 {
        self.bus
    }

    #[must_use]
    #[inline]
    pub const fn device(self) -> u8 {
        self.device
    }

    #[must_use]
    #[inline]
    pub const fn function(self) -> u8 {
        self.function
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    pub(super) id: u16,
    pub(super) vendor_id: u16,
    pub(super) bdf: BdfAddress,
    pub(super) functions: u8,
    pub(super) csp: Csp,
    pub(super) revision: u8,
}

impl Device {
    #[must_use]
    #[inline]
    pub const fn id(&self) -> u16 {
        self.id
    }

    #[must_use]
    #[inline]
    pub const fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    #[must_use]
    #[inline]
    pub const fn bdf(&self) -> BdfAddress {
        self.bdf
    }

    #[must_use]
    #[inline]
    pub const fn functions(&self) -> u8 {
        self.functions
    }

    #[must_use]
    #[inline]
    pub const fn csp(&self) -> Csp {
        self.csp
    }

    #[must_use]
    #[inline]
    pub const fn revision(&self) -> u8 {
        self.revision
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigAddressValue {
    /// Bit 31: Enable bit
    pub(super) enable: bool,
    /// Bit 30-24: Reserved
    /// Bit 23-16: Bus number
    /// Bit 15-11: Device number
    /// Bit 10-8: Function number
    pub(super) bdf: BdfAddress,
    /// Bit 7-0: Register offset
    ///
    /// For accesses, offset must be DWORD-aligned.
    pub(super) register_offset: u8,
}

impl ConfigAddressValue {
    #[must_use]
    pub const fn new(bus: u8, device: u8, function: u8, register_offset: u8) -> Self {
        // Here, we are not checking that register_offset is DWORD-aligned.
        Self {
            enable: true,
            bdf: BdfAddress::new(bus, device, function),
            register_offset,
        }
    }

    #[must_use]
    pub fn as_raw(self) -> u32 {
        let enable_bit = u32::from(self.enable) << 31;
        let bus = u32::from(self.bdf.bus()) << 16;
        let device = u32::from(self.bdf.device()) << 11;
        let function = u32::from(self.bdf.function()) << 8;
        let register_offset = u32::from(self.register_offset);

        enable_bit | bus | device | function | register_offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bar {
    Memory(MemoryBar),
    Io(IoBar),
}

impl Bar {
    #[must_use]
    pub fn from_raw(value: u32, device: &Device, bar_number: u8) -> Self {
        if value & 0b1 == 0 {
            Self::Memory(MemoryBar::from_raw(value, device, bar_number))
        } else {
            Self::Io(IoBar::from_raw(value))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBar {
    /// 16-byte aligned
    base_address: PhysAddr,
    /// If the memory is not prefetchable, caching must be disabled.
    ///
    /// Thus, it is better to access memory with volatile reads and writes.
    prefetchable: bool,
}

impl MemoryBar {
    #[must_use]
    fn from_raw(value: u32, device: &Device, bar_number: u8) -> Self {
        assert_eq!(value & 0b1, 0, "BAR register is not memory type");
        let prefetchable = (value & 0b100) != 0;
        let bar_type = MemoryBarType::try_from((value >> 1) & 0b11).unwrap();
        // FIXME: Refactor this
        let upper_value = if bar_type == MemoryBarType::Qword {
            let bar_reg_offset = match bar_number + 1 {
                0 => RegisterOffset::Bar0,
                1 => RegisterOffset::Bar1,
                2 => RegisterOffset::Bar2,
                3 => RegisterOffset::Bar3,
                4 => RegisterOffset::Bar4,
                5 => RegisterOffset::Bar5,
                _ => panic!("PCI: Invalid BAR number"),
            } as u8;
            let bar_reg = ConfigAddressValue::new(
                device.bdf().bus(),
                device.bdf().device(),
                device.bdf().function(),
                bar_reg_offset,
            );
            todo!("Implement 64-bit read");
            // LEGACY_PCI_HANDLER
            //     .with_locked(|handler| handler.read_u32(bar_reg))
            //     .unwrap()
        } else {
            0_u32
        };
        let base_address =
            PhysAddr::new((u64::from(upper_value) << 32) | (u64::from(value & 0xFFFF_FFF0)));

        Self {
            base_address,
            prefetchable,
        }
    }

    #[must_use]
    #[inline]
    pub const fn base_address(&self) -> PhysAddr {
        self.base_address
    }

    #[must_use]
    #[inline]
    pub const fn prefetchable(&self) -> bool {
        self.prefetchable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoBar {
    /// 4-byte aligned
    base_address: PhysAddr,
}

impl IoBar {
    #[must_use]
    pub fn from_raw(value: u32) -> Self {
        assert_eq!(value & 0b1, 1, "BAR register is not IO type");
        let base_address = PhysAddr::new(u64::from(value & 0xFFFF_FFFC));

        Self { base_address }
    }

    #[must_use]
    #[inline]
    pub const fn base_address(self) -> PhysAddr {
        self.base_address
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBarType {
    Dword,
    Qword,
}

impl TryFrom<u32> for MemoryBarType {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0b00 => Ok(Self::Dword),
            0b10 => Ok(Self::Qword),
            _ => Err(()),
        }
    }
}
