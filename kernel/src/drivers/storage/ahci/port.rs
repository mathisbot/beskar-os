//! AHCI Port Management
//!
//! Each port represents a single SATA device connection.

use super::{
    super::super::DmaPage,
    command::AhciCommand,
    registers::{PortRegisters, SataDet},
};
use crate::mem::vmm;
use beskar_core::{
    arch::{
        VirtAddr,
        paging::{Frame, M4KiB, MemSize as _, Page},
    },
    drivers::{DriverError, DriverResult},
};

/// Timeout for port operations (in iterations)
pub const PORT_TIMEOUT: usize = 1_000_000_000;
const PORT_CMD_ST: u32 = 1 << 0;
const PORT_CMD_FRE: u32 = 1 << 4;
const PORT_CMD_FR: u32 = 1 << 14;
const PORT_CMD_CR: u32 = 1 << 15;
const COMMAND_LIST_LEN: usize = 1024;
const RECEIVED_FIS_LEN: usize = 256;
const COMMAND_TABLE_LEN: usize = 256;
const COMMAND_SLOT: u32 = 1;
const PORT_TFD_BSY: u32 = 1 << 7;
const PORT_TFD_DRQ: u32 = 1 << 3;
const PORT_TFD_ERR: u32 = 1 << 0;
const PORT_TFD_DF: u32 = 1 << 5;
const PORT_IS_TFES: u32 = 1 << 30;
pub const ATA_SECTOR_SIZE: usize = 512;
const MAX_SECTORS_PER_COMMAND: u16 = 8;
const PRDT_OFFSET: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataDirection {
    DeviceToHost,
    HostToDevice,
}

impl DataDirection {
    #[must_use]
    #[inline]
    const fn is_write(self) -> bool {
        matches!(self, Self::HostToDevice)
    }
}

/// Represents a single AHCI port with an attached device
pub struct AhciPort {
    regs: PortRegisters,
    port_id: u32,
    resources: Option<PortResources>,
}

struct PortResources {
    command_list: DmaPage,
    received_fis: DmaPage,
    command_table: DmaPage,
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
            .set_clb(resources.command_list.phys_addr().as_u64());
        self.regs
            .set_fb(resources.received_fis.phys_addr().as_u64());
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
        (tfd & (PORT_TFD_ERR | PORT_TFD_DF)) != 0
    }

    #[inline]
    /// Clear port errors
    pub fn clear_errors(&self) {
        self.regs.set_sata_error(u32::MAX);
        self.regs.set_is(u32::MAX);
    }

    pub fn identify(&mut self) -> DriverResult<IdentifyDevice> {
        let mut data = [0_u8; ATA_SECTOR_SIZE];
        self.read_command(&AhciCommand::identify_device(), &mut data)?;
        Ok(IdentifyDevice::parse(&data))
    }

    pub fn read_sectors(&mut self, lba: u64, buffer: &mut [u8]) -> DriverResult<()> {
        if !buffer.len().is_multiple_of(ATA_SECTOR_SIZE) {
            return Err(DriverError::Invalid);
        }

        let mut current_lba = lba;
        for chunk in buffer.chunks_mut(usize::from(MAX_SECTORS_PER_COMMAND) * ATA_SECTOR_SIZE) {
            let sectors = u16::try_from(chunk.len() / ATA_SECTOR_SIZE).unwrap();
            let command = AhciCommand::read_dma_ext(current_lba, sectors);
            self.read_command(&command, chunk)?;
            current_lba += u64::from(sectors);
        }

        Ok(())
    }

    pub fn write_sectors(&mut self, lba: u64, buffer: &[u8]) -> DriverResult<()> {
        if !buffer.len().is_multiple_of(ATA_SECTOR_SIZE) {
            return Err(DriverError::Invalid);
        }

        let mut current_lba = lba;
        for chunk in buffer.chunks(usize::from(MAX_SECTORS_PER_COMMAND) * ATA_SECTOR_SIZE) {
            let sectors = u16::try_from(chunk.len() / ATA_SECTOR_SIZE).unwrap();
            let command = AhciCommand::write_dma_ext(current_lba, sectors);
            self.write_command(&command, chunk)?;
            current_lba += u64::from(sectors);
        }

        Ok(())
    }

    fn read_command(&mut self, command: &AhciCommand, buffer: &mut [u8]) -> DriverResult<()> {
        let data = DmaPage::new(buffer.len())?;
        let result =
            self.submit_synchronous_data_command(command, &data, DataDirection::DeviceToHost);
        if result.is_ok() {
            data.copy_to_slice(buffer);
        }
        result
    }

    fn write_command(&mut self, command: &AhciCommand, buffer: &[u8]) -> DriverResult<()> {
        let data = DmaPage::new(buffer.len())?;
        data.copy_from_slice(buffer);
        self.submit_synchronous_data_command(command, &data, DataDirection::HostToDevice)
    }

    fn submit_synchronous_data_command(
        &mut self,
        command: &AhciCommand,
        data: &DmaPage,
        direction: DataDirection,
    ) -> DriverResult<()> {
        if data.len() == 0 || data.len() > usize::try_from(M4KiB::SIZE).unwrap() {
            return Err(DriverError::Invalid);
        }

        self.prepare_command(command, data, direction)?;
        self.issue_command(COMMAND_SLOT);
        self.wait_for_command_completion(COMMAND_SLOT)
    }

    fn prepare_command(
        &mut self,
        command: &AhciCommand,
        data: &DmaPage,
        direction: DataDirection,
    ) -> DriverResult<()> {
        self.wait_until_ready()?;
        self.clear_errors();

        let resources = self.resources.as_mut().ok_or(DriverError::Unknown)?;
        resources.clear_command_buffers();
        resources.write_command_header(direction);
        resources.write_command_table(command, data);

        Ok(())
    }

    #[inline]
    fn issue_command(&self, slot: u32) {
        self.regs.set_ci(slot);
    }

    fn wait_for_command_completion(&self, slot: u32) -> DriverResult<()> {
        let mut timeout = PORT_TIMEOUT;
        while self.regs.ci() & slot != 0 {
            if self.has_task_file_error() {
                crate::warn!("AHCI port {} task file error", self.port_id);
                return Err(DriverError::Unknown);
            }

            timeout -= 1;
            if timeout == 0 {
                crate::warn!("AHCI port {} command timeout", self.port_id);
                return Err(DriverError::Unknown);
            }
            core::hint::spin_loop();
        }

        if self.has_task_file_error() || self.is_error() {
            crate::warn!("AHCI port {} command failed", self.port_id);
            return Err(DriverError::Unknown);
        }

        Ok(())
    }

    fn wait_until_ready(&self) -> DriverResult<()> {
        let mut timeout = PORT_TIMEOUT;
        while self.regs.tfd() & (PORT_TFD_BSY | PORT_TFD_DRQ) != 0 {
            timeout -= 1;
            if timeout == 0 {
                crate::warn!("AHCI port {} busy timeout", self.port_id);
                return Err(DriverError::Unknown);
            }
            core::hint::spin_loop();
        }

        Ok(())
    }

    #[must_use]
    #[inline]
    fn has_task_file_error(&self) -> bool {
        self.regs.is() & PORT_IS_TFES != 0
    }
}

impl PortResources {
    fn new() -> DriverResult<Self> {
        let command_list = DmaPage::new(COMMAND_LIST_LEN)?;
        let received_fis = DmaPage::new(RECEIVED_FIS_LEN)?;
        let command_table = DmaPage::new(COMMAND_TABLE_LEN)?;

        Ok(Self {
            command_list,
            received_fis,
            command_table,
        })
    }

    const fn clear_command_buffers(&self) {
        self.command_list.clear();
        self.command_table.clear();
    }

    fn write_command_header(&self, direction: DataDirection) {
        let fis_len = u8::try_from(size_of::<super::FisH2D>().div_ceil(4)).unwrap();
        let flags = CommandHeader::flags(fis_len, direction.is_write());
        let command_table_addr = self.command_table.phys_addr().as_u64();
        let base = self.command_list.as_mut_ptr::<u8>();

        unsafe {
            write_volatile_u16(base, flags);
            write_volatile_u16(base.byte_add(2), 1);
            write_volatile_u32(base.byte_add(4), 0);
            write_volatile_u32(
                base.byte_add(8),
                u32::try_from(command_table_addr & 0xFFFF_FFFF).unwrap(),
            );
            write_volatile_u32(
                base.byte_add(12),
                u32::try_from((command_table_addr >> 32) & 0xFFFF_FFFF).unwrap(),
            );
        }
    }

    fn write_command_table(&self, command: &AhciCommand, data: &DmaPage) {
        let table = self.command_table.as_mut_ptr::<u8>();
        let data_addr = data.phys_addr().as_u64();
        let byte_count = u32::try_from(data.len()).unwrap() - 1;

        unsafe {
            write_volatile_bytes(
                table,
                core::ptr::from_ref(command.fis()).cast::<u8>(),
                size_of::<super::FisH2D>(),
            );

            let prd = table.byte_add(PRDT_OFFSET);
            write_volatile_u32(prd, u32::try_from(data_addr & 0xFFFF_FFFF).unwrap());
            write_volatile_u32(
                prd.byte_add(4),
                u32::try_from((data_addr >> 32) & 0xFFFF_FFFF).unwrap(),
            );
            write_volatile_u32(prd.byte_add(8), 0);
            write_volatile_u32(prd.byte_add(12), byte_count | (1 << 31));
        }
    }
}

unsafe fn write_volatile_bytes(dst: *mut u8, src: *const u8, len: usize) {
    for offset in 0..len {
        unsafe { dst.add(offset).write_volatile(src.add(offset).read()) };
    }
}

unsafe fn write_volatile_u16(dst: *mut u8, value: u16) {
    unsafe { write_volatile_bytes(dst, value.to_le_bytes().as_ptr(), size_of::<u16>()) };
}

unsafe fn write_volatile_u32(dst: *mut u8, value: u32) {
    unsafe { write_volatile_bytes(dst, value.to_le_bytes().as_ptr(), size_of::<u32>()) };
}

#[derive(Debug, Clone, Copy)]
pub struct IdentifyDevice {
    sector_count: u64,
}

impl IdentifyDevice {
    #[must_use]
    fn parse(data: &[u8; ATA_SECTOR_SIZE]) -> Self {
        let word = |idx: usize| -> u16 {
            let offset = idx * 2;
            u16::from_le_bytes([data[offset], data[offset + 1]])
        };

        let lba48_supported = word(83) & (1 << 10) != 0;
        let sector_count = if lba48_supported {
            u64::from(word(100))
                | (u64::from(word(101)) << 16)
                | (u64::from(word(102)) << 32)
                | (u64::from(word(103)) << 48)
        } else {
            u64::from(word(60)) | (u64::from(word(61)) << 16)
        };

        Self { sector_count }
    }

    #[must_use]
    #[inline]
    pub const fn sector_count(self) -> u64 {
        self.sector_count
    }
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
    pub const fn new(fis_len: u8, write: bool, command_table_addr: u64) -> Self {
        let mut header = Self {
            cmd_fis_len_flags: 0,
            prdt_len: 1,
            prd_byte_count: 0,
            ctba_low: 0,
            ctba_high: 0,
            _reserved: [0; 4],
        };
        header.set_fis_length(fis_len);
        header.set_write(write);
        header.set_ctba(command_table_addr);
        header
    }

    #[must_use]
    #[inline]
    const fn flags(fis_len: u8, write: bool) -> u16 {
        let mut flags = fis_len as u16;
        if write {
            flags |= 1 << 6;
        }
        flags
    }

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
    pub fn new(data_addr: u64, byte_count: usize) -> Self {
        let mut entry = Self {
            dba_low: 0,
            dba_high: 0,
            _reserved: 0,
            dbc_ioc: 0,
        };
        entry.set_dba(data_addr);
        entry.set_byte_count(u32::try_from(byte_count).unwrap() - 1);
        entry.set_ioc(true);
        entry
    }

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
