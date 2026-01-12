//! AHCI Port Management
//!
//! Each port represents a single SATA device connection.

use super::registers::{PortRegisters, SataDet};
use beskar_core::{
    arch::VirtAddr,
    drivers::{DriverError, DriverResult},
};

/// Timeout for port operations (in iterations)
pub const PORT_TIMEOUT: usize = 1_000_000_000;

/// Represents a single AHCI port with an attached device
pub struct AhciPort {
    regs: PortRegisters,
    port_id: u32,
}

impl AhciPort {
    #[must_use]
    #[inline]
    /// Create a new AHCI port instance
    pub const fn new(base: VirtAddr, port_id: u32) -> Self {
        let regs = unsafe { PortRegisters::from_base(base) };
        Self { regs, port_id }
    }

    #[must_use]
    #[inline]
    /// Check if a device is present on this port
    pub fn is_device_present(&self) -> bool {
        let sata_status = self.regs.sata_status();
        let det = SataDet::from_bits(sata_status);
        matches!(det, SataDet::DevicePresent | SataDet::DevicePresentComm)
    }

    /// Initialize the AHCI port
    pub fn initialize(&self) -> DriverResult<()> {
        // Clear any pending errors
        let sata_error = self.regs.sata_error();
        if sata_error != 0 {
            self.regs.set_sata_error(sata_error);
        }

        // Wait for device to become ready
        let mut timeout = PORT_TIMEOUT;
        loop {
            let sata_status = self.regs.sata_status();
            let det = SataDet::from_bits(sata_status);

            match det {
                SataDet::DevicePresentComm => break,
                SataDet::NoDevice => return Err(DriverError::Absent),
                _ => {}
            }

            timeout -= 1;
            if timeout == 0 {
                video::warn!("AHCI port {} device ready timeout", self.port_id);
                return Err(DriverError::Unknown);
            }
        }

        // Clear interrupt status
        let is = self.regs.is();
        if is != 0 {
            self.regs.set_is(is);
        }

        // Enable port interrupts
        self.regs.set_ie(0xFDC0_00FF);

        // Start command engine
        self.start_command_engine()?;

        video::debug!(
            "AHCI port {} initialized (status=0x{:08x})",
            self.port_id,
            self.regs.sata_status()
        );

        Ok(())
    }

    /// Start the port's command engine
    fn start_command_engine(&self) -> DriverResult<()> {
        // Clear command list base address first
        self.regs.set_clb(0);
        self.regs.set_fb(0);

        let mut cmd = self.regs.cmd();

        // Set start (ST) and FIS receive enable (FRE) bits
        cmd |= 0x0001; // ST
        cmd |= 0x0010; // FRE

        self.regs.set_cmd(cmd);

        // Verify command engine started
        let mut timeout = PORT_TIMEOUT;
        loop {
            let cmd = self.regs.cmd();
            if (cmd & 0x0001) != 0 {
                break;
            }

            timeout -= 1;
            if timeout == 0 {
                video::warn!("AHCI port {} command engine start timeout", self.port_id);
                return Err(DriverError::Unknown);
            }
        }

        Ok(())
    }

    /// Stop the port's command engine
    pub fn stop_command_engine(&self) -> DriverResult<()> {
        let mut cmd = self.regs.cmd();

        // Clear start (ST) and FIS receive enable (FRE) bits
        cmd &= !0x0001; // Clear ST
        cmd &= !0x0010; // Clear FRE

        self.regs.set_cmd(cmd);

        // Verify command engine stopped
        let mut timeout = PORT_TIMEOUT;
        loop {
            let cmd = self.regs.cmd();
            if (cmd & 0x0001) == 0 {
                break;
            }

            timeout -= 1;
            if timeout == 0 {
                video::warn!("AHCI port {} command engine stop timeout", self.port_id);
                return Err(DriverError::Unknown);
            }
        }

        Ok(())
    }

    #[must_use]
    #[inline]
    /// Get the port ID
    pub const fn id(&self) -> u32 {
        self.port_id
    }

    #[must_use]
    #[inline]
    /// Get SATA status
    pub fn sata_status(&self) -> u32 {
        self.regs.sata_status()
    }

    #[must_use]
    #[inline]
    /// Get device detection status
    pub fn device_detection(&self) -> SataDet {
        let sata_status = self.regs.sata_status();
        SataDet::from_bits(sata_status)
    }

    #[must_use]
    #[inline]
    /// Get task file data
    pub fn task_file_data(&self) -> u32 {
        self.regs.tfd()
    }

    #[must_use]
    #[inline]
    /// Check if port has errors
    pub fn is_error(&self) -> bool {
        let tfd = self.regs.tfd();
        (tfd & 0xFF) != 0 // Status register, error bits
    }

    #[inline]
    /// Clear port errors
    pub fn clear_errors(&self) {
        self.regs.set_sata_error(u32::MAX);
        self.regs.set_is(u32::MAX);
    }
}

/// Port command list entry header
#[repr(C, packed)]
pub struct CommandHeader {
    /// Bit 0: Command FIS length (in DWORDs)
    /// Bits 5: Write (1=Host to device)
    /// Bits 10-15: Port multiplier port
    pub cmd_fis_len_flags: u16,
    /// Physical region descriptor table length
    pub prdt_len: u16,
    /// Physical region descriptor byte count
    pub prd_byte_count: u32,
    /// Command table base address (lower 32-bits)
    pub ctba_low: u32,
    /// Command table base address (upper 32-bits)
    pub ctba_high: u32,
    _reserved: [u32; 4],
}

impl CommandHeader {
    #[must_use]
    #[inline]
    /// Get FIS length in DWORDs
    pub const fn fis_length(&self) -> u8 {
        (self.cmd_fis_len_flags & 0x1F) as u8
    }

    #[inline]
    /// Set FIS length in DWORDs
    pub const fn set_fis_length(&mut self, len: u8) {
        self.cmd_fis_len_flags = (self.cmd_fis_len_flags & !0x1F) | (len as u16);
    }

    #[must_use]
    #[inline]
    /// Check if this is a write (host to device)
    pub const fn is_write(&self) -> bool {
        (self.cmd_fis_len_flags & (1 << 6)) != 0
    }

    #[inline]
    /// Set write flag
    pub const fn set_write(&mut self, write: bool) {
        if write {
            self.cmd_fis_len_flags |= 1 << 6;
        } else {
            self.cmd_fis_len_flags &= !(1 << 6);
        }
    }

    #[must_use]
    #[inline]
    /// Get command table address (48-bit physical address)
    pub const fn ctba(&self) -> u64 {
        ((self.ctba_high as u64) << 32) | (self.ctba_low as u64)
    }

    #[inline]
    /// Set command table address
    pub const fn set_ctba(&mut self, addr: u64) {
        self.ctba_low = (addr & 0xFFFF_FFFF) as u32;
        self.ctba_high = ((addr >> 32) & 0xFFFF_FFFF) as u32;
    }
}

/// Received FIS structure (typically 256 bytes per port)
#[repr(C, packed)]
pub struct ReceivedFis {
    pub dma_setup: [u8; 28],
    _pad1: [u8; 4],
    pub pio_setup: [u8; 20],
    _pad2: [u8; 4],
    pub d2h_register: [u8; 20],
    _pad3: [u8; 4],
    pub set_device_bits: [u8; 8],
    pub unknown_fis: [u8; 64],
    _reserved: [u8; 96],
}

/// Physical Region Descriptor Table entry
#[repr(C, packed)]
pub struct PrdTableEntry {
    /// Data base address (lower 32-bits)
    pub dba_low: u32,
    /// Data base address (upper 32-bits)
    pub dba_high: u32,
    _reserved: u32,
    /// Bits 21-0: byte count (0 means 4MB)
    /// Bit 31: Interrupt on completion
    pub dbc_ioc: u32,
}

impl PrdTableEntry {
    #[must_use]
    #[inline]
    /// Get data buffer address (48-bit physical address)
    pub const fn dba(&self) -> u64 {
        ((self.dba_high as u64) << 32) | (self.dba_low as u64)
    }

    #[inline]
    /// Set data buffer address
    pub fn set_dba(&mut self, addr: u64) {
        self.dba_low = u32::try_from(addr & 0xFFFF_FFFF).unwrap();
        self.dba_high = u32::try_from((addr >> 32) & 0xFFFF_FFFF).unwrap();
    }

    #[must_use]
    #[inline]
    /// Get byte count
    pub const fn byte_count(&self) -> u32 {
        let bc = self.dbc_ioc & 0x3F_FFFF;
        if bc == 0 { 0x40_0000 } else { bc }
    }

    #[inline]
    /// Set byte count
    pub fn set_byte_count(&mut self, count: u32) {
        let count = count.min(0x40_0000);
        self.dbc_ioc = (self.dbc_ioc & 0xFFC0_0000) | (count & 0x3F_FFFF);
    }

    #[must_use]
    #[inline]
    /// Check interrupt on completion flag
    pub const fn ioc(&self) -> bool {
        (self.dbc_ioc & (1 << 31)) != 0
    }

    #[inline]
    /// Set interrupt on completion flag
    pub const fn set_ioc(&mut self, ioc: bool) {
        if ioc {
            self.dbc_ioc |= 1 << 31;
        } else {
            self.dbc_ioc &= !(1 << 31);
        }
    }
}
