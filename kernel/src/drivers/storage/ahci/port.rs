//! AHCI Port Management
//!
//! Each port represents a single SATA device connection.

use super::registers::{PortRegisters, SataDet};
use crate::mem::vmm;
use beskar_core::{
    arch::{
        VirtAddr,
        paging::{Frame, M4KiB, MemSize as _, Page},
    },
    drivers::{DriverError, DriverResult},
};
use beskar_hal::paging::page_table::Flags;

/// Timeout for port operations (in iterations)
pub const PORT_TIMEOUT: usize = 1_000_000_000;
const PORT_CMD_ST: u32 = 1 << 0;
const PORT_CMD_FRE: u32 = 1 << 4;
const PORT_CMD_FR: u32 = 1 << 14;
const PORT_CMD_CR: u32 = 1 << 15;
const COMMAND_LIST_LEN: usize = 1024;
const RECEIVED_FIS_LEN: usize = 256;

/// Represents a single AHCI port with an attached device
pub struct AhciPort {
    regs: PortRegisters,
    port_id: u32,
    resources: Option<PortResources>,
}

struct PortResources {
    command_list_page: Page<M4KiB>,
    command_list_frame: Frame<M4KiB>,
    received_fis_page: Page<M4KiB>,
    received_fis_frame: Frame<M4KiB>,
}

impl AhciPort {
    #[must_use]
    #[inline]
    /// Create a new AHCI port instance
    pub const fn new(base: VirtAddr, port_id: u32) -> Self {
        let regs = unsafe { PortRegisters::from_base(base) };
        Self {
            regs,
            port_id,
            resources: None,
        }
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
                crate::warn!("AHCI port {} device ready timeout", self.port_id);
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

        self.stop_command_engine()?;
        self.install_dma_buffers()?;

        // Start command engine
        self.start_command_engine()?;

        crate::debug!(
            "AHCI port {} initialized (status=0x{:08x})",
            self.port_id,
            self.regs.sata_status()
        );

        Ok(())
    }

    /// Start the port's command engine
    fn start_command_engine(&self) -> DriverResult<()> {
        if self.resources.is_none() {
            return Err(DriverError::Unknown);
        }

        let mut cmd = self.regs.cmd();

        // Set start (ST) and FIS receive enable (FRE) bits
        cmd |= PORT_CMD_ST;
        cmd |= PORT_CMD_FRE;

        self.regs.set_cmd(cmd);

        // Verify command engine started
        let mut timeout = PORT_TIMEOUT;
        loop {
            let cmd = self.regs.cmd();
            if (cmd & PORT_CMD_ST) != 0 {
                break;
            }

            timeout -= 1;
            if timeout == 0 {
                crate::warn!("AHCI port {} command engine start timeout", self.port_id);
                return Err(DriverError::Unknown);
            }
        }

        Ok(())
    }

    /// Stop the port's command engine
    pub fn stop_command_engine(&self) -> DriverResult<()> {
        let mut cmd = self.regs.cmd();

        // Clear start (ST) and FIS receive enable (FRE) bits
        cmd &= !PORT_CMD_ST;
        cmd &= !PORT_CMD_FRE;

        self.regs.set_cmd(cmd);

        // Verify command engine stopped
        let mut timeout = PORT_TIMEOUT;
        loop {
            let cmd = self.regs.cmd();
            if (cmd & (PORT_CMD_ST | PORT_CMD_FRE | PORT_CMD_FR | PORT_CMD_CR)) == 0 {
                break;
            }

            timeout -= 1;
            if timeout == 0 {
                crate::warn!("AHCI port {} command engine stop timeout", self.port_id);
                return Err(DriverError::Unknown);
            }
        }

        Ok(())
    }

    fn install_dma_buffers(&mut self) -> DriverResult<()> {
        let resources = PortResources::new()?;
        self.regs
            .set_clb(resources.command_list_frame.start_address().as_u64());
        self.regs
            .set_fb(resources.received_fis_frame.start_address().as_u64());
        self.resources = Some(resources);
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

impl PortResources {
    fn new() -> DriverResult<Self> {
        let (command_list_page, command_list_frame) = allocate_dma_page(COMMAND_LIST_LEN)?;
        let (received_fis_page, received_fis_frame) = allocate_dma_page(RECEIVED_FIS_LEN)?;

        Ok(Self {
            command_list_page,
            command_list_frame,
            received_fis_page,
            received_fis_frame,
        })
    }
}

impl Drop for PortResources {
    fn drop(&mut self) {
        free_dma_page(self.command_list_page, self.command_list_frame);
        free_dma_page(self.received_fis_page, self.received_fis_frame);
    }
}

fn allocate_dma_page(length: usize) -> DriverResult<(Page<M4KiB>, Frame<M4KiB>)> {
    debug_assert!(length <= usize::try_from(M4KiB::SIZE).unwrap());

    let Some(page) = vmm::kernel::reserve_pages::<M4KiB>(1).map(|range| range.start()) else {
        return Err(DriverError::Unknown);
    };
    let page_range = Page::range_inclusive(page, page);

    let Some(frame) = vmm::kernel::alloc_frame::<M4KiB>() else {
        vmm::kernel::free_pages(page_range);
        return Err(DriverError::Unknown);
    };

    if vmm::kernel::map_frame(page, frame, Flags::MMIO_SUITABLE).is_err() {
        vmm::kernel::free_frame(frame);
        vmm::kernel::free_pages(page_range);
        return Err(DriverError::Unknown);
    }

    unsafe { core::ptr::write_bytes(page.start_address().as_mut_ptr::<u8>(), 0, length) };

    Ok((page, frame))
}

fn free_dma_page(page: Page<M4KiB>, fallback_frame: Frame<M4KiB>) {
    if let Ok(frame) = vmm::kernel::unmap_page(page) {
        vmm::kernel::free_frame(frame);
    } else {
        vmm::kernel::free_frame(fallback_frame);
    }
    vmm::kernel::free_pages(Page::range_inclusive(page, page));
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
