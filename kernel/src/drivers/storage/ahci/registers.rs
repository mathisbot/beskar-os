//! AHCI Controller Registers
//!
//! This module provides access to the AHCI controller's memory-mapped I/O registers.

use beskar_core::arch::VirtAddr;
use core::ptr::{read_volatile, write_volatile};

/// AHCI Controller Registers
pub struct AhciRegisters {
    base: VirtAddr,
}

impl AhciRegisters {
    /// Create registers accessor from base address
    ///
    /// # Safety
    ///
    /// The base address must point to valid AHCI controller MMIO memory.
    pub unsafe fn from_base(base: VirtAddr) -> Self {
        Self { base }
    }

    fn read_u32(&self, offset: u32) -> u32 {
        unsafe { read_volatile(self.base.as_ptr::<u32>().byte_add(offset as usize)) }
    }

    fn write_u32(&self, offset: u32, value: u32) {
        unsafe {
            write_volatile(
                self.base.as_mut_ptr::<u32>().byte_add(offset as usize),
                value,
            )
        }
    }

    /// Capabilities register (0x00)
    pub fn capabilities(&self) -> Capabilities {
        Capabilities(self.read_u32(0x00))
    }

    /// Global Host Control register (0x04)
    pub fn ghc(&self) -> u32 {
        self.read_u32(0x04)
    }

    pub fn set_ghc(&self, value: u32) {
        self.write_u32(0x04, value);
    }

    /// Interrupt Status register (0x08)
    pub fn interrupt_status(&self) -> u32 {
        self.read_u32(0x08)
    }

    pub fn set_interrupt_status(&self, value: u32) {
        self.write_u32(0x08, value);
    }

    /// Ports Implemented register (0x0C)
    pub fn ports_implemented(&self) -> u32 {
        self.read_u32(0x0C)
    }

    /// AHCI Version register (0x10)
    pub fn version(&self) -> u32 {
        self.read_u32(0x10)
    }

    /// Command Completion Coalescing Control register (0x14)
    pub fn ccc_ctl(&self) -> u32 {
        self.read_u32(0x14)
    }

    /// Command Completion Coalescing Ports register (0x18)
    pub fn ccc_ports(&self) -> u32 {
        self.read_u32(0x18)
    }

    /// Enclosure Management Location register (0x1C)
    pub fn em_loc(&self) -> u32 {
        self.read_u32(0x1C)
    }

    /// Enclosure Management Control register (0x20)
    pub fn em_ctl(&self) -> u32 {
        self.read_u32(0x20)
    }

    /// Capabilities Extended register (0x24)
    pub fn capabilities_ext(&self) -> u32 {
        self.read_u32(0x24)
    }

    /// BIOS/OS Handoff Control and Status register (0x28)
    pub fn bohc(&self) -> u32 {
        self.read_u32(0x28)
    }
}

/// AHCI Capabilities Register (0x00)
#[derive(Debug, Copy, Clone)]
pub struct Capabilities(u32);

impl Capabilities {
    /// Number of Ports
    pub fn np(&self) -> u32 {
        (self.0 & 0x1F) as u32
    }

    /// Supports External SATA
    pub fn sxs(&self) -> bool {
        (self.0 & (1 << 5)) != 0
    }

    /// Enclosure Management Supported
    pub fn ems(&self) -> bool {
        (self.0 & (1 << 6)) != 0
    }

    /// Command Completion Coalescing Supported
    pub fn cccs(&self) -> bool {
        (self.0 & (1 << 7)) != 0
    }

    /// Number of Command Slots
    pub fn ncs(&self) -> u32 {
        ((self.0 >> 8) & 0x1F) + 1
    }

    /// Command Set Switch (CSS)
    pub fn css(&self) -> u32 {
        (self.0 >> 16) & 0xFF
    }

    /// Supports SATA Port Multiplier
    pub fn spm(&self) -> bool {
        (self.0 & (1 << 17)) != 0
    }

    /// FIS-based Switching Supported
    pub fn fbss(&self) -> bool {
        (self.0 & (1 << 16)) != 0
    }

    /// PIO Multiple DRQ Block
    pub fn pmd(&self) -> bool {
        (self.0 & (1 << 24)) != 0
    }

    /// Slumber State Capable
    pub fn ssc(&self) -> bool {
        (self.0 & (1 << 25)) != 0
    }

    /// Partial State Capable
    pub fn psc(&self) -> bool {
        (self.0 & (1 << 26)) != 0
    }

    /// Supports 64-bit Addressing
    pub fn sam(&self) -> bool {
        (self.0 & (1 << 31)) != 0
    }
}

/// AHCI Port Registers (at offset 0x100 + portnum*0x80)
pub struct PortRegisters {
    base: VirtAddr,
}

impl PortRegisters {
    /// Create port registers accessor
    ///
    /// # Safety
    ///
    /// The base address must point to valid AHCI port MMIO memory.
    pub unsafe fn from_base(base: VirtAddr) -> Self {
        Self { base }
    }

    fn read_u32(&self, offset: u32) -> u32 {
        unsafe { read_volatile(self.base.as_ptr::<u32>().byte_add(offset as usize)) }
    }

    fn write_u32(&self, offset: u32, value: u32) {
        unsafe {
            write_volatile(
                self.base.as_mut_ptr::<u32>().byte_add(offset as usize),
                value,
            )
        }
    }

    fn read_u64(&self, offset: u32) -> u64 {
        unsafe { read_volatile(self.base.as_ptr::<u64>().byte_add(offset as usize)) }
    }

    fn write_u64(&self, offset: u32, value: u64) {
        unsafe {
            write_volatile(
                self.base.as_mut_ptr::<u64>().byte_add(offset as usize),
                value,
            )
        }
    }

    /// Command List Base Address (0x00-0x03)
    pub fn clb(&self) -> u64 {
        self.read_u64(0x00)
    }

    pub fn set_clb(&self, value: u64) {
        self.write_u64(0x00, value);
    }

    /// Received FIS Base Address (0x08-0x0B)
    pub fn fb(&self) -> u64 {
        self.read_u64(0x08)
    }

    pub fn set_fb(&self, value: u64) {
        self.write_u64(0x08, value);
    }

    /// Interrupt Status (0x10)
    pub fn is(&self) -> u32 {
        self.read_u32(0x10)
    }

    pub fn set_is(&self, value: u32) {
        self.write_u32(0x10, value);
    }

    /// Interrupt Enable (0x14)
    pub fn ie(&self) -> u32 {
        self.read_u32(0x14)
    }

    pub fn set_ie(&self, value: u32) {
        self.write_u32(0x14, value);
    }

    /// Command and Status (0x18)
    pub fn cmd(&self) -> u32 {
        self.read_u32(0x18)
    }

    pub fn set_cmd(&self, value: u32) {
        self.write_u32(0x18, value);
    }

    /// Task File Data (0x20)
    pub fn tfd(&self) -> u32 {
        self.read_u32(0x20)
    }

    /// Serial ATA Status (0x28)
    pub fn sata_status(&self) -> u32 {
        self.read_u32(0x28)
    }

    /// Serial ATA Control (0x2C)
    pub fn sata_control(&self) -> u32 {
        self.read_u32(0x2C)
    }

    pub fn set_sata_control(&self, value: u32) {
        self.write_u32(0x2C, value);
    }

    /// Serial ATA Error (0x30)
    pub fn sata_error(&self) -> u32 {
        self.read_u32(0x30)
    }

    pub fn set_sata_error(&self, value: u32) {
        self.write_u32(0x30, value);
    }

    /// Serial ATA Active (0x34)
    pub fn sata_active(&self) -> u32 {
        self.read_u32(0x34)
    }

    /// Command Issue (0x38)
    pub fn ci(&self) -> u32 {
        self.read_u32(0x38)
    }

    pub fn set_ci(&self, value: u32) {
        self.write_u32(0x38, value);
    }

    /// SATA Notification (0x3C)
    pub fn sntf(&self) -> u32 {
        self.read_u32(0x3C)
    }

    pub fn set_sntf(&self, value: u32) {
        self.write_u32(0x3C, value);
    }

    /// FIS-based Switching Control (0x40)
    pub fn fbs(&self) -> u32 {
        self.read_u32(0x40)
    }

    pub fn set_fbs(&self, value: u32) {
        self.write_u32(0x40, value);
    }
}

/// Decode SATA Port Detection (SPD) bits from sata_status
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SataSpd {
    NoDevice = 0,
    Gen1 = 1,
    Gen2 = 2,
    Gen3 = 3,
    Reserved = 4,
}

impl SataSpd {
    pub fn from_bits(bits: u32) -> Self {
        match bits & 0xF {
            0 => SataSpd::NoDevice,
            1 => SataSpd::Gen1,
            2 => SataSpd::Gen2,
            3 => SataSpd::Gen3,
            _ => SataSpd::Reserved,
        }
    }
}

/// Decode SATA Device Detection (DET) bits from sata_status
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SataDet {
    NoDevice = 0,
    DevicePresent = 1,
    DevicePresentComm = 3,
    OfflineMode = 4,
}

impl SataDet {
    pub fn from_bits(bits: u32) -> Self {
        match bits & 0xF {
            0 => SataDet::NoDevice,
            1 => SataDet::DevicePresent,
            3 => SataDet::DevicePresentComm,
            4 => SataDet::OfflineMode,
            _ => SataDet::NoDevice,
        }
    }
}
