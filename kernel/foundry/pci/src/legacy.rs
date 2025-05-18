//! Legacy PCI handling module.

use super::commons::{Class, Csp, Device, PciAddress, RegisterOffset, SbdfAddress};
use alloc::vec::Vec;
use beskar_hal::port::{Port, ReadWrite, WriteOnly};

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

#[derive(Debug, Default, Clone)]
pub struct LegacyPciHandler {
    config_port: ConfigAddress,
    data_port: ConfigData,
    devices: Vec<super::commons::Device>,
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
        for bus in 0..=255 {
            for device in 0..32 {
                if let Some(device) = self.scan_device(PciAddress::new(
                    0,
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
    fn scan_device(&mut self, address: PciAddress) -> Option<Device> {
        let (device, vendor) = {
            let vendor_reg = PciAddress {
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
            let class_reg = PciAddress {
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
            sbdf: address.sbdf,
            functions,
            csp: Csp::new(class, subclass, prog_if),
            revision,
            segment_group_number: 0,
        })
    }

    #[must_use]
    fn find_function_count(&mut self, address: PciAddress) -> u8 {
        let header_reg = PciAddress {
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
                    let vendor_reg = PciAddress {
                        register_offset: RegisterOffset::VendorId as u8,
                        sbdf: SbdfAddress::new(0, address.sbdf.bus(), address.sbdf.device(), func),
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
    unsafe fn read_raw(&mut self, address: PciAddress) -> u32 {
        let PciAddress {
            enable: _,
            sbdf,
            register_offset,
        } = address;

        let aligned_offset = register_offset & !0b11;
        let unaligned = register_offset & 0b11;

        let aligned_address = PciAddress {
            enable: true,
            sbdf,
            register_offset: aligned_offset,
        };

        unsafe {
            self.config_port
                .write(ConfigAddress::build_value(aligned_address));
        };
        let value = unsafe { self.data_port.read() };

        value >> (unaligned * 8)
    }

    fn write_raw(&mut self, address: PciAddress, value: u32) {
        assert!(
            address.register_offset.trailing_zeros() >= 2,
            "Unaligned write"
        );

        unsafe {
            self.config_port.write(ConfigAddress::build_value(address));
        };
        unsafe {
            self.data_port.write(value);
        };
    }
}

macro_rules! impl_read_u {
    ($name:ident, $t:ty) => {
        impl LegacyPciHandler {
            #[must_use]
            fn $name(&mut self, address: PciAddress) -> Option<$t> {
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

    fn read_raw(&mut self, address: PciAddress) -> u32 {
        self.read_u32(address).unwrap()
    }

    fn write_raw(&mut self, address: PciAddress, value: u32) {
        self.write_raw(address, value);
    }
}

#[derive(Debug, Clone)]
/// Configuration address PCI register
///
/// This is a write-only register that is used to select the register to access.
pub struct ConfigAddress(Port<u32, WriteOnly>);

impl Default for ConfigAddress {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigAddress {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self(Port::new(CONFIG_ADDRESS))
    }

    #[must_use]
    fn build_value(address: PciAddress) -> u32 {
        let enable_bit = u32::from(address.enable) << 31;
        let bus = u32::from(address.sbdf.bus()) << 16;
        let device = u32::from(address.sbdf.device()) << 11;
        let function = u32::from(address.sbdf.function()) << 8;
        let register_offset = u32::from(address.register_offset);

        enable_bit | bus | device | function | register_offset
    }
}

impl core::ops::Deref for ConfigAddress {
    type Target = Port<u32, WriteOnly>;

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
struct ConfigData(Port<u32, ReadWrite>);

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
    type Target = Port<u32, ReadWrite>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for ConfigData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
