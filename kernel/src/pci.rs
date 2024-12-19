// TODO: PCI Express support

use crate::utils::locks::McsLock;
use alloc::vec::Vec;
use x86_64::{
    instructions::port::{Port, PortWriteOnly},
    structures::port::PortWrite,
    PhysAddr,
};

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

static PCI_HANDLER: McsLock<PciHandler> = McsLock::new(PciHandler::new());

pub struct PciHandler {
    config_port: ConfigAddress,
    data_port: ConfigData,
    devices: Vec<Device>,
}

pub fn init() {
    PCI_HANDLER.with_locked(|handler| {
        handler.update_devices();
        if handler.devices.is_empty() {
            log::warn!("No PCI devices found");
        } else {
            log::debug!("Found {} PCI devices", handler.devices.len());
        }
    });
}

impl PciHandler {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            config_port: ConfigAddress::new(),
            data_port: ConfigData::new(),
            devices: Vec::new(),
        }
    }

    #[must_use]
    #[inline]
    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub fn update_devices(&mut self) {
        self.devices.clear();

        // Brute-force scan
        // FIXME: Way too slow (8192 iterations)
        for bus in 0..=255 {
            for device in 0..32 {
                if let Some(device) = self.scan_device(ConfigAddressValue::new(
                    bus,
                    device,
                    0,
                    RegisterOffset::VendorId as u8,
                )) {
                    self.devices.push(device);
                }
            }
        }
    }

    #[must_use]
    fn scan_device(&mut self, address: ConfigAddressValue) -> Option<Device> {
        let (device, vendor) = {
            let vendor_reg = ConfigAddressValue {
                register_offset: RegisterOffset::VendorId as u8,
                ..address
            };
            let vendor = self.read_u32(vendor_reg)?;
            (
                u16::try_from(vendor >> 16).unwrap(),
                u16::try_from(vendor & 0xFFFF).unwrap(),
            )
        };

        let (class, subclass, prog_if, revision) = {
            let class_reg = ConfigAddressValue {
                register_offset: RegisterOffset::RevisionId as u8,
                ..address
            };
            let class_reg_value = self.read_u32(class_reg)?;
            (
                Class::from(u8::try_from(class_reg_value >> 24).unwrap()),
                u8::try_from((class_reg_value >> 16) & 0xFF).unwrap(),
                u8::try_from((class_reg_value >> 8) & 0xFF).unwrap(),
                u8::try_from(class_reg_value & 0xFF).unwrap(),
            )
        };

        let functions = self.find_function_count(address);

        Some(Device {
            id: device,
            vendor_id: vendor,
            bdf: address.bdf,
            functions,
            csp: Csp {
                class,
                subclass,
                prog_if,
            },
            revision,
        })
    }

    #[must_use]
    fn find_function_count(&mut self, address: ConfigAddressValue) -> u8 {
        let header_reg = ConfigAddressValue {
            register_offset: RegisterOffset::HeaderType as u8,
            ..address
        };
        let header = self.read_u8(header_reg).unwrap();

        // Check multi-function bit
        if (header >> 7) == 0 {
            return 1;
        }

        // Brute-force over functions
        u8::try_from(
            (1..8)
                .take_while(|&func| {
                    let vendor_reg = ConfigAddressValue {
                        register_offset: RegisterOffset::VendorId as u8,
                        bdf: BdfAddress::new(address.bdf.bus(), address.bdf.device(), func),
                        ..address
                    };
                    self.read_u16(vendor_reg).is_some()
                })
                .count(),
        )
        .unwrap()
            + 1
    }

    #[must_use]
    /// Read the raw value from the PCI configuration space
    ///
    /// This function should not be used directly as the value is not validated.
    /// Instead, use the `read_u*` functions.
    unsafe fn read_raw(&mut self, address: ConfigAddressValue) -> u32 {
        let ConfigAddressValue {
            enable: _,
            bdf,
            register_offset,
        } = address;

        let aligned_offset = register_offset & !0b11;
        let unaligned = register_offset & 0b11;

        let aligned_address = ConfigAddressValue {
            enable: true,
            bdf,
            register_offset: aligned_offset,
        };

        unsafe {
            self.config_port.write(aligned_address.as_raw());
        };
        let value = unsafe { self.data_port.read() };

        value >> (unaligned * 8)
    }
}

macro_rules! impl_read_u {
    ($name:ident, $t:ty) => {
        impl PciHandler {
            #[must_use]
            fn $name(&mut self, address: ConfigAddressValue) -> Option<$t> {
                let raw_value = unsafe { self.read_raw(address) };
                let value = <$t>::try_from((raw_value) & u32::from(<$t>::MAX)).unwrap();
                if value == <$t>::MAX {
                    None
                } else {
                    Some(value)
                }
            }
        }
    };
}

impl_read_u!(read_u8, u8);
impl_read_u!(read_u16, u16);
impl_read_u!(read_u32, u32);

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
    pub const fn bus(&self) -> u8 {
        self.bus
    }

    #[must_use]
    #[inline]
    pub const fn device(&self) -> u8 {
        self.device
    }

    #[must_use]
    #[inline]
    pub const fn function(&self) -> u8 {
        self.function
    }
}

#[derive(Debug, Clone)]
/// Configuration address PCI register
///
/// This is a write-only register that is used to select the register to access.
pub struct ConfigAddress(PortWriteOnly<u32>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConfigAddressValue {
    /// Bit 31: Enable bit
    enable: bool,
    /// Bit 30-24: Reserved
    /// Bit 23-16: Bus number
    /// Bit 15-11: Device number
    /// Bit 10-8: Function number
    bdf: BdfAddress,
    /// Bit 7-0: Register offset
    ///
    /// For accesses, offset must be DWORD-aligned.
    register_offset: u8,
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
    pub fn as_raw(&self) -> u32 {
        let enable_bit = u32::from(self.enable) << 31;
        let bus = u32::from(self.bdf.bus()) << 16;
        let device = u32::from(self.bdf.device()) << 11;
        let function = u32::from(self.bdf.function()) << 8;
        let register_offset = u32::from(self.register_offset);

        enable_bit | bus | device | function | register_offset
    }
}

impl PortWrite for ConfigAddressValue {
    unsafe fn write_to_port(port: u16, value: Self) {
        unsafe {
            u32::write_to_port(port, value.as_raw());
        }
    }
}

impl ConfigAddress {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self(PortWriteOnly::new(CONFIG_ADDRESS))
    }
}

impl core::ops::Deref for ConfigAddress {
    type Target = PortWriteOnly<u32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for ConfigAddress {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone)]
/// Configuration address PCI register
///
/// This is a write-only register that is used to select the register to access.
struct ConfigData(Port<u32>);

impl ConfigData {
    pub const fn new() -> Self {
        Self(Port::new(CONFIG_DATA))
    }
}

impl core::ops::Deref for ConfigData {
    type Target = Port<u32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for ConfigData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    id: u16,
    vendor_id: u16,
    bdf: BdfAddress,
    functions: u8,
    csp: Csp,
    revision: u8,
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

    #[must_use]
    /// Read the raw value from the PCI configuration space
    ///
    /// Bar number must be 0 to 5 (inclusive).
    pub fn bar(&self, bar_number: u8) -> Option<Bar> {
        let bar_reg_offset = match bar_number {
            0 => RegisterOffset::Bar0,
            1 => RegisterOffset::Bar1,
            2 => RegisterOffset::Bar2,
            3 => RegisterOffset::Bar3,
            4 => RegisterOffset::Bar4,
            5 => RegisterOffset::Bar5,
            _ => panic!("Invalid BAR number"),
        } as u8;
        let bar_reg = ConfigAddressValue::new(
            self.bdf().bus(),
            self.bdf().device(),
            self.bdf().function(),
            bar_reg_offset,
        );
        let bar = PCI_HANDLER.with_locked(|handler| handler.read_u32(bar_reg));
        bar.map(|bar| Bar::from_raw(bar, self, bar_number))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bar {
    Memory(MemoryBar),
    Io(IoBar),
}

impl Bar {
    #[must_use]
    fn from_raw(value: u32, device: &Device, bar_number: u8) -> Self {
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
        let bar_type = match (value >> 1) & 0b11 {
            0b00 => MemoryBarType::Dword,
            0b10 => MemoryBarType::Qword,
            x => panic!("PCI: Invalid Memory BAR type: {}", x),
        };
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
                device.bdf.bus(),
                device.bdf.device(),
                device.bdf.function(),
                bar_reg_offset,
            );
            PCI_HANDLER
                .with_locked(|handler| handler.read_u32(bar_reg))
                .unwrap()
        } else {
            0
        };
        let base_address =
            PhysAddr::new(u64::from(upper_value) << 32 | u64::from(value & 0xFFFFFFF0));

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
        let base_address = PhysAddr::new(u64::from(value & 0xFFFFFFFC));

        Self { base_address }
    }

    #[must_use]
    #[inline]
    pub const fn base_address(&self) -> PhysAddr {
        self.base_address
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBarType {
    Dword,
    Qword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Device Class, Subclass, ProgIf
pub struct Csp {
    class: Class,
    subclass: u8,
    prog_if: u8,
}

impl Csp {
    #[must_use]
    #[inline]
    pub const fn class(&self) -> Class {
        self.class
    }

    #[must_use]
    #[inline]
    pub const fn subclass(&self) -> u8 {
        self.subclass
    }

    #[must_use]
    #[inline]
    pub const fn prog_if(&self) -> u8 {
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
enum RegisterOffset {
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

pub fn with_pci_handler<T, F: FnOnce(&mut PciHandler) -> T>(f: F) -> T {
    PCI_HANDLER.with_locked(f)
}
