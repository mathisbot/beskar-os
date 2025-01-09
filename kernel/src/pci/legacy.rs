//! Legacy PCI handling module.

use alloc::vec::Vec;
use hyperdrive::locks::mcs::McsLock;
use x86_64::{
    instructions::port::{Port, PortWriteOnly},
    structures::port::PortWrite,
};

use super::commons::{
    Bar, BdfAddress, Class, ConfigAddressValue, Csp, Device, MemoryBarType, RegisterOffset,
};

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

static LEGACY_PCI_HANDLER: McsLock<LegacyPciHandler> = McsLock::new(LegacyPciHandler::new());

#[derive(Debug, Default, Clone)]
pub struct LegacyPciHandler {
    config_port: ConfigAddress,
    data_port: ConfigData,
    devices: Vec<super::commons::Device>,
}

pub fn init() {
    with_legacy_pci_handler(|handler| {
        handler.update_devices();
        if handler.devices.is_empty() {
            crate::warn!("No PCI devices found");
        } else {
            crate::debug!("Found {} PCI devices", handler.devices.len());
        }
    });
}

impl LegacyPciHandler {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            config_port: ConfigAddress::new(),
            data_port: ConfigData::new(),
            devices: Vec::new(),
        }
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
            csp: Csp::new(class, subclass, prog_if),
            revision,
            segment_group_number: 0,
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
                .filter(|&func| {
                    let vendor_reg = ConfigAddressValue {
                        register_offset: RegisterOffset::VendorId as u8,
                        bdf: BdfAddress::new(address.bdf.bus(), address.bdf.device(), func),
                        ..address
                    };

                    // Vendor ID is 0xFFFF if function is unsupported
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
        impl LegacyPciHandler {
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

impl super::PciHandler for LegacyPciHandler {
    fn devices(&self) -> &[super::commons::Device] {
        &self.devices
    }

    fn read_bar(&mut self, device: &Device, bar_number: u8) -> Option<Bar> {
        let bar_reg_offset = match bar_number {
            0 => RegisterOffset::Bar0,
            1 => RegisterOffset::Bar1,
            2 => RegisterOffset::Bar2,
            3 => RegisterOffset::Bar3,
            4 => RegisterOffset::Bar4,
            5 => RegisterOffset::Bar5,
            _ => return None,
        } as u8;
        let bar_reg = ConfigAddressValue::new(
            device.bdf().bus(),
            device.bdf().device(),
            device.bdf().function(),
            bar_reg_offset,
        );
        let bar = self.read_u32(bar_reg)?;

        let upper_value = if bar & 1 == 0 // Memory BAR
            && MemoryBarType::try_from((bar >> 1) & 0b11).unwrap() == MemoryBarType::Qword
        {
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
            self.read_u32(bar_reg).unwrap()
        } else {
            0
        };

        Some(Bar::from_raw(
            u64::from(bar) | (u64::from(upper_value) << 32),
        ))
    }
}

#[derive(Debug, Clone)]
/// Configuration address PCI register
///
/// This is a write-only register that is used to select the register to access.
pub struct ConfigAddress(PortWriteOnly<u32>);

impl PortWrite for ConfigAddressValue {
    unsafe fn write_to_port(port: u16, value: Self) {
        unsafe {
            u32::write_to_port(port, value.as_raw());
        }
    }
}

impl Default for ConfigAddress {
    fn default() -> Self {
        Self::new()
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

impl Default for ConfigData {
    fn default() -> Self {
        Self::new()
    }
}

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

pub fn with_legacy_pci_handler<T, F: FnOnce(&mut LegacyPciHandler) -> T>(f: F) -> T {
    LEGACY_PCI_HANDLER.with_locked(f)
}