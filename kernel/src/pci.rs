#![allow(dead_code, unused_variables)] // TODO: Remove

use alloc::vec::Vec;
use x86_64::instructions::port::{Port, PortGeneric, WriteOnlyAccess};

const CONFIG_ADDRESS: u32 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

pub fn init() {
    // TODO: Implement
}

pub struct PciHandler {
    port: PortGeneric<ConfigAddress, WriteOnlyAccess>,
    devices: Vec<Device>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    id: u16,
    vendor_id: u16,
    address: Address,
    function: u8,
    class: Class,
    revision: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Storage,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address {
    /// The bus number.
    bus: u8,
    /// The device number is the 5-bit device number
    /// within the bus.
    ///
    /// Only bits 0-4 are used.
    /// All other bits should be 0.
    device: u8,
}

impl From<Address> for u32 {
    fn from(address: Address) -> Self {
        let mut n = 0;
        n |= Self::from(address.bus);
        n |= Self::from(address.device) << 16;
        n
    }
}

impl Address {
    #[must_use]
    #[inline]
    pub fn new(bus: u8, device: u8) -> Self {
        assert_eq!(device >> 5, 0, "Device number should be 5 bits long");

        Self { bus, device }
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigAddress {
    address: Address,
    /// The function number is the 3-bit function number
    /// within the device.
    ///
    /// Only bits 0-2 are used.
    /// All other bits should be 0.
    function_number: u8,
    /// The register offset within the configuration space.
    ///
    /// As it should point to consecutive DWORDs, bits 0-1
    /// should be 0.
    register: u8,
}

impl From<ConfigAddress> for u32 {
    fn from(value: ConfigAddress) -> Self {
        let mut n = Self::from(value.address);
        n |= Self::from(value.function_number) << 8;
        n |= Self::from(value.register) << 12;
        n |= 1 << 31; // Enable bit
        n
    }
}

impl ConfigAddress {
    #[must_use]
    #[inline]
    pub fn new(address: Address, function_number: u8, register: u8) -> Self {
        assert_eq!(
            function_number >> 3,
            0,
            "Function number should be 3 bits long"
        );
        assert_eq!(register & 0b11, 0, "Register should be DWORD aligned");

        Self {
            address,
            function_number,
            register,
        }
    }

    #[must_use]
    #[inline]
    pub const fn data_port() -> Port<u32> {
        Port::new(CONFIG_DATA)
    }

    #[must_use]
    #[inline]
    pub const fn address(self) -> Address {
        self.address
    }

    #[must_use]
    #[inline]
    pub const fn function_number(self) -> u8 {
        self.function_number
    }

    #[must_use]
    #[inline]
    pub const fn register_offset(self) -> u8 {
        self.register
    }
}
