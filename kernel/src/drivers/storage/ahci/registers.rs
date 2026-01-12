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
    #[must_use]
    #[inline]
    /// Create registers accessor from base address
    ///
    /// # Safety
    ///
    /// The base address must point to valid AHCI controller MMIO memory.
    pub const unsafe fn from_base(base: VirtAddr) -> Self {
        Self { base }
    }

    #[must_use]
    #[inline]
    fn read_u32(&self, offset: u32) -> u32 {
        unsafe { read_volatile(self.base.as_ptr::<u32>().byte_add(offset as usize)) }
    }

    #[inline]
    fn write_u32(&self, offset: u32, value: u32) {
        unsafe {
            write_volatile(
                self.base.as_mut_ptr::<u32>().byte_add(offset as usize),
                value,
            );
        }
    }

    #[must_use]
    #[inline]
    /// Capabilities register (0x00)
    pub fn capabilities(&self) -> Capabilities {
        Capabilities(self.read_u32(0x00))
    }

    #[must_use]
    #[inline]
    /// Global Host Control register (0x04)
    pub fn ghc(&self) -> u32 {
        self.read_u32(0x04)
    }

    #[inline]
    pub fn set_ghc(&self, value: u32) {
        self.write_u32(0x04, value);
    }

    #[must_use]
    #[inline]
    /// Interrupt Status register (0x08)
    pub fn interrupt_status(&self) -> u32 {
        self.read_u32(0x08)
    }

    #[inline]
    pub fn set_interrupt_status(&self, value: u32) {
        self.write_u32(0x08, value);
    }

    #[must_use]
    #[inline]
    /// Ports Implemented register (0x0C)
    pub fn ports_implemented(&self) -> u32 {
        self.read_u32(0x0C)
    }

    #[must_use]
    #[inline]
    /// AHCI Version register (0x10)
    pub fn version(&self) -> u32 {
        self.read_u32(0x10)
    }

    #[must_use]
    #[inline]
    /// Command Completion Coalescing Control register (0x14)
    pub fn ccc_ctl(&self) -> u32 {
        self.read_u32(0x14)
    }

    #[must_use]
    #[inline]
    /// Command Completion Coalescing Ports register (0x18)
    pub fn ccc_ports(&self) -> u32 {
        self.read_u32(0x18)
    }

    #[must_use]
    #[inline]
    /// Enclosure Management Location register (0x1C)
    pub fn em_loc(&self) -> u32 {
        self.read_u32(0x1C)
    }

    #[must_use]
    #[inline]
    /// Enclosure Management Control register (0x20)
    pub fn em_ctl(&self) -> u32 {
        self.read_u32(0x20)
    }

    #[must_use]
    #[inline]
    /// Capabilities Extended register (0x24)
    pub fn capabilities_ext(&self) -> u32 {
        self.read_u32(0x24)
    }

    #[must_use]
    #[inline]
    /// BIOS/OS Handoff Control and Status register (0x28)
    pub fn bohc(&self) -> u32 {
        self.read_u32(0x28)
    }
}

/// AHCI Capabilities Register (0x00)
#[derive(Debug, Copy, Clone)]
pub struct Capabilities(u32);

impl Capabilities {
    #[must_use]
    #[inline]
    /// Number of Ports
    pub const fn np(self) -> u32 {
        self.0 & 0x1F
    }

    #[must_use]
    #[inline]
    /// Supports External SATA
    pub const fn sxs(self) -> bool {
        (self.0 & (1 << 5)) != 0
    }

    #[must_use]
    #[inline]
    /// Enclosure Management Supported
    pub const fn ems(self) -> bool {
        (self.0 & (1 << 6)) != 0
    }

    #[must_use]
    #[inline]
    /// Command Completion Coalescing Supported
    pub const fn cccs(self) -> bool {
        (self.0 & (1 << 7)) != 0
    }

    #[must_use]
    #[inline]
    /// Number of Command Slots
    pub const fn ncs(self) -> u32 {
        ((self.0 >> 8) & 0x1F) + 1
    }

    #[must_use]
    #[inline]
    /// Command Set Switch (CSS)
    pub const fn css(self) -> u32 {
        (self.0 >> 16) & 0xFF
    }

    #[must_use]
    #[inline]
    /// Supports SATA Port Multiplier
    pub const fn spm(self) -> bool {
        (self.0 & (1 << 17)) != 0
    }

    #[must_use]
    #[inline]
    /// FIS-based Switching Supported
    pub const fn fbss(self) -> bool {
        (self.0 & (1 << 16)) != 0
    }

    #[must_use]
    #[inline]
    /// PIO Multiple DRQ Block
    pub const fn pmd(self) -> bool {
        (self.0 & (1 << 24)) != 0
    }

    #[must_use]
    #[inline]
    /// Slumber State Capable
    pub const fn ssc(self) -> bool {
        (self.0 & (1 << 25)) != 0
    }

    #[must_use]
    #[inline]
    /// Partial State Capable
    pub const fn psc(self) -> bool {
        (self.0 & (1 << 26)) != 0
    }

    #[must_use]
    #[inline]
    /// Supports 64-bit Addressing
    pub const fn sam(self) -> bool {
        (self.0 & (1 << 31)) != 0
    }
}

/// AHCI Port Registers (at offset 0x100 + portnum*0x80)
pub struct PortRegisters {
    base: VirtAddr,
}

impl PortRegisters {
    #[must_use]
    #[inline]
    /// Create port registers accessor
    ///
    /// # Safety
    ///
    /// The base address must point to valid AHCI port MMIO memory.
    pub const unsafe fn from_base(base: VirtAddr) -> Self {
        Self { base }
    }

    #[must_use]
    #[inline]
    fn read_u32(&self, offset: u32) -> u32 {
        unsafe { read_volatile(self.base.as_ptr::<u32>().byte_add(offset as usize)) }
    }

    #[inline]
    fn write_u32(&self, offset: u32, value: u32) {
        unsafe {
            write_volatile(
                self.base.as_mut_ptr::<u32>().byte_add(offset as usize),
                value,
            );
        }
    }

    #[must_use]
    #[inline]
    fn read_u64(&self, offset: u32) -> u64 {
        unsafe { read_volatile(self.base.as_ptr::<u64>().byte_add(offset as usize)) }
    }

    #[inline]
    fn write_u64(&self, offset: u32, value: u64) {
        unsafe {
            write_volatile(
                self.base.as_mut_ptr::<u64>().byte_add(offset as usize),
                value,
            );
        }
    }

    #[must_use]
    #[inline]
    /// Command List Base Address (0x00-0x03)
    pub fn clb(&self) -> u64 {
        self.read_u64(0x00)
    }

    #[inline]
    pub fn set_clb(&self, value: u64) {
        self.write_u64(0x00, value);
    }

    #[must_use]
    #[inline]
    /// Received FIS Base Address (0x08-0x0B)
    pub fn fb(&self) -> u64 {
        self.read_u64(0x08)
    }

    #[inline]
    pub fn set_fb(&self, value: u64) {
        self.write_u64(0x08, value);
    }

    #[must_use]
    #[inline]
    /// Interrupt Status (0x10)
    pub fn is(&self) -> u32 {
        self.read_u32(0x10)
    }

    #[inline]
    pub fn set_is(&self, value: u32) {
        self.write_u32(0x10, value);
    }

    #[must_use]
    #[inline]
    /// Interrupt Enable (0x14)
    pub fn ie(&self) -> u32 {
        self.read_u32(0x14)
    }

    #[inline]
    pub fn set_ie(&self, value: u32) {
        self.write_u32(0x14, value);
    }

    #[must_use]
    #[inline]
    /// Command and Status (0x18)
    pub fn cmd(&self) -> u32 {
        self.read_u32(0x18)
    }

    #[inline]
    pub fn set_cmd(&self, value: u32) {
        self.write_u32(0x18, value);
    }

    #[must_use]
    #[inline]
    /// Task File Data (0x20)
    pub fn tfd(&self) -> u32 {
        self.read_u32(0x20)
    }

    #[must_use]
    #[inline]
    /// Serial ATA Status (0x28)
    pub fn sata_status(&self) -> u32 {
        self.read_u32(0x28)
    }

    #[must_use]
    #[inline]
    /// Serial ATA Control (0x2C)
    pub fn sata_control(&self) -> u32 {
        self.read_u32(0x2C)
    }

    #[inline]
    pub fn set_sata_control(&self, value: u32) {
        self.write_u32(0x2C, value);
    }

    #[must_use]
    #[inline]
    /// Serial ATA Error (0x30)
    pub fn sata_error(&self) -> u32 {
        self.read_u32(0x30)
    }

    #[inline]
    pub fn set_sata_error(&self, value: u32) {
        self.write_u32(0x30, value);
    }

    #[must_use]
    #[inline]
    /// Serial ATA Active (0x34)
    pub fn sata_active(&self) -> u32 {
        self.read_u32(0x34)
    }

    #[must_use]
    #[inline]
    /// Command Issue (0x38)
    pub fn ci(&self) -> u32 {
        self.read_u32(0x38)
    }

    #[inline]
    pub fn set_ci(&self, value: u32) {
        self.write_u32(0x38, value);
    }

    #[must_use]
    #[inline]
    /// SATA Notification (0x3C)
    pub fn sntf(&self) -> u32 {
        self.read_u32(0x3C)
    }

    #[inline]
    pub fn set_sntf(&self, value: u32) {
        self.write_u32(0x3C, value);
    }

    #[must_use]
    #[inline]
    /// FIS-based Switching Control (0x40)
    pub fn fbs(&self) -> u32 {
        self.read_u32(0x40)
    }

    #[inline]
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
    #[must_use]
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        match bits & 0xF {
            0 => Self::NoDevice,
            1 => Self::Gen1,
            2 => Self::Gen2,
            3 => Self::Gen3,
            _ => Self::Reserved,
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
    #[must_use]
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        match bits & 0xF {
            1 => Self::DevicePresent,
            3 => Self::DevicePresentComm,
            4 => Self::OfflineMode,
            _ => Self::NoDevice,
        }
    }
}
