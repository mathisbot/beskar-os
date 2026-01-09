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
    /// Create a new AHCI port instance
    pub fn new(base: VirtAddr, port_id: u32) -> DriverResult<Self> {
        let regs = unsafe { PortRegisters::from_base(base) };
        Ok(Self { regs, port_id })
    }

    /// Check if a device is present on this port
    pub fn is_device_present(&self) -> DriverResult<bool> {
        let sata_status = self.regs.sata_status();
        let det = SataDet::from_bits(sata_status);

        match det {
            SataDet::DevicePresent | SataDet::DevicePresentComm => Ok(true),
            _ => Ok(false),
        }
    }

    /// Initialize the AHCI port
    pub fn initialize(&mut self) -> DriverResult<()> {
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
        self.regs.set_ie(0xFDC000FF);

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

    /// Get the port ID
    pub fn id(&self) -> u32 {
        self.port_id
    }

    /// Get SATA status
    pub fn sata_status(&self) -> u32 {
        self.regs.sata_status()
    }

    /// Get device detection status
    pub fn device_detection(&self) -> SataDet {
        let sata_status = self.regs.sata_status();
        SataDet::from_bits(sata_status)
    }

    /// Get task file data
    pub fn task_file_data(&self) -> u32 {
        self.regs.tfd()
    }

    /// Check if port has errors
    pub fn is_error(&self) -> bool {
        let tfd = self.regs.tfd();
        (tfd & 0xFF) != 0 // Status register, error bits
    }

    /// Clear port errors
    pub fn clear_errors(&self) {
        self.regs.set_sata_error(0xFFFFFFFF);
        self.regs.set_is(0xFFFFFFFF);
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
    /// Get FIS length in DWORDs
    pub fn fis_length(&self) -> u8 {
        (self.cmd_fis_len_flags & 0x1F) as u8
    }

    /// Set FIS length in DWORDs
    pub fn set_fis_length(&mut self, len: u8) {
        self.cmd_fis_len_flags = (self.cmd_fis_len_flags & !0x1F) | (len as u16);
    }

    /// Check if this is a write (host to device)
    pub fn is_write(&self) -> bool {
        (self.cmd_fis_len_flags & (1 << 6)) != 0
    }

    /// Set write flag
    pub fn set_write(&mut self, write: bool) {
        if write {
            self.cmd_fis_len_flags |= 1 << 6;
        } else {
            self.cmd_fis_len_flags &= !(1 << 6);
        }
    }

    /// Get command table address (48-bit physical address)
    pub fn ctba(&self) -> u64 {
        ((self.ctba_high as u64) << 32) | (self.ctba_low as u64)
    }

    /// Set command table address
    pub fn set_ctba(&mut self, addr: u64) {
        self.ctba_low = (addr & 0xFFFFFFFF) as u32;
        self.ctba_high = ((addr >> 32) & 0xFFFFFFFF) as u32;
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
    /// Get data buffer address (48-bit physical address)
    pub fn dba(&self) -> u64 {
        ((self.dba_high as u64) << 32) | (self.dba_low as u64)
    }

    /// Set data buffer address
    pub fn set_dba(&mut self, addr: u64) {
        self.dba_low = (addr & 0xFFFFFFFF) as u32;
        self.dba_high = ((addr >> 32) & 0xFFFFFFFF) as u32;
    }

    /// Get byte count
    pub fn byte_count(&self) -> u32 {
        let bc = self.dbc_ioc & 0x3FFFFF;
        if bc == 0 { 0x400000 } else { bc }
    }

    /// Set byte count
    pub fn set_byte_count(&mut self, count: u32) {
        let count = if count > 0x400000 {
            0x400000
        } else if count == 0 {
            0
        } else {
            count
        };
        self.dbc_ioc = (self.dbc_ioc & 0xFFC00000) | (count & 0x3FFFFF);
    }

    /// Check interrupt on completion flag
    pub fn ioc(&self) -> bool {
        (self.dbc_ioc & (1 << 31)) != 0
    }

    /// Set interrupt on completion flag
    pub fn set_ioc(&mut self, ioc: bool) {
        if ioc {
            self.dbc_ioc |= 1 << 31;
        } else {
            self.dbc_ioc &= !(1 << 31);
        }
    }
}
