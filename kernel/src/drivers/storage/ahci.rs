use crate::mem::vmm::phys_map::PhysicalMapping;
use ::pci::{Bar, Device};
use alloc::vec::Vec;
use beskar_core::{
    arch::{VirtAddr, paging::M4KiB},
    drivers::{DriverError, DriverResult},
    storage::{BlockDevice, BlockDeviceError},
};
use beskar_hal::paging::page_table::Flags;
use hyperdrive::locks::mcs::MUMcsLock;

mod command;
mod fis;
pub use fis::{AtaCommand, FisD2H, FisH2D, FisType};
mod port;
use port::AhciPort;
mod registers;
use registers::AhciRegisters;

/// Timeout for controller operations (in iterations)
const CONTROLLER_TIMEOUT: usize = 100_000_000;
const GENERIC_HOST_CONTROL_LEN: usize = 0x100;
const PORT_REG_OFFSET: usize = 0x100;
const PORT_REG_SIZE: usize = 0x80;

static AHCI: MUMcsLock<Ahci> = MUMcsLock::uninit();

/// AHCI Global Host Control Register
const GHC_OFFSET: u32 = 0x04;
/// AHCI version register
const VS_OFFSET: u32 = 0x00;
/// Number of ports register
const PI_OFFSET: u32 = 0x0C;
/// Controller capabilities register
const CAP_OFFSET: u32 = 0x00;

pub fn init(ahci_controllers: &[Device]) -> DriverResult<()> {
    let Some(controller) = ahci_controllers.first() else {
        return Err(DriverError::Absent);
    };

    let Some(Bar::Memory(bar)) =
        crate::drivers::pci::with_pci_handler(|handler| handler.read_bar(controller, 5))
    else {
        return Err(DriverError::Absent);
    };

    let ahci_paddr = bar.base_address();
    let flags = Flags::MMIO_SUITABLE;

    let port_count = {
        let pmap = PhysicalMapping::<M4KiB>::new(ahci_paddr, GENERIC_HOST_CONTROL_LEN, flags)
            .map_err(|_| DriverError::Unknown)?;
        let ahci_base = pmap.translate(ahci_paddr).ok_or(DriverError::Unknown)?;
        let regs = unsafe { AhciRegisters::from_base(ahci_base) };
        usize::try_from(regs.capabilities().np() + 1).unwrap()
    };

    let required_len = PORT_REG_OFFSET + port_count * PORT_REG_SIZE;
    let pmap = PhysicalMapping::<M4KiB>::new(ahci_paddr, required_len, flags)
        .map_err(|_| DriverError::Unknown)?;

    let ahci_base = pmap.translate(ahci_paddr).ok_or(DriverError::Unknown)?;

    let mut ahci = Ahci::new(ahci_base, pmap);
    ahci.initialize()?;
    AHCI.init(ahci);

    crate::info!("AHCI controller initialized successfully");

    Ok(())
}

/// AHCI controller instance
pub struct Ahci {
    base: VirtAddr,
    pmap: PhysicalMapping,
    /// Number of ports supported by this controller
    port_count: u32,
    ports: Vec<AhciPort>,
}

impl Ahci {
    /// Create a new AHCI controller instance
    fn new(base: VirtAddr, pmap: PhysicalMapping) -> Self {
        let regs = unsafe { AhciRegisters::from_base(base) };

        let version = regs.version();
        let capabilities = regs.capabilities();
        let port_count = capabilities.np() + 1;

        crate::debug!("AHCI version: {}.{}", version >> 16, version & 0xFFFF);
        crate::debug!("AHCI supports {} ports", port_count);

        Self {
            base,
            pmap,
            port_count,
            ports: Vec::new(),
        }
    }

    /// Initialize the AHCI controller
    fn initialize(&mut self) -> DriverResult<()> {
        let regs = unsafe { AhciRegisters::from_base(self.base) };

        // Enable AHCI mode (set AE bit in GHC)
        let ghc = regs.ghc();
        regs.set_ghc(ghc | 0x8000_0000);

        // Wait for AHCI mode to be enabled
        let mut timeout = CONTROLLER_TIMEOUT;
        while (regs.ghc() & 0x8000_0000) == 0 && timeout > 0 {
            timeout -= 1;
        }
        if timeout == 0 {
            crate::warn!("AHCI mode enable timeout");
            return Err(DriverError::Unknown);
        }

        // Enable interrupts (set IE bit in GHC)
        let ghc = regs.ghc();
        regs.set_ghc(ghc | 0x2);

        // Detect and initialize ports
        self.probe_ports()?;

        Ok(())
    }

    /// Probe all AHCI ports and initialize any attached drives
    fn probe_ports(&mut self) -> DriverResult<()> {
        let regs = unsafe { AhciRegisters::from_base(self.base) };
        let ports_implemented = regs.ports_implemented();

        for port_idx in 0..self.port_count {
            // Check if this port is implemented
            if (ports_implemented & (1 << port_idx)) == 0 {
                continue;
            }

            let port_offset = 0x100 + (u64::from(port_idx) * 0x80);
            let port_addr = self.base + port_offset;
            let mut port = AhciPort::new(port_addr, port_idx);

            // Check if a device is present
            if port.is_device_present() {
                port.initialize()?;
                crate::debug!("AHCI port {} initialized with device", port_idx);
                self.ports.push(port);
            }
        }

        Ok(())
    }
}

struct AhciDisk {
    port: AhciPort,
    sector_count: u64,
}

impl BlockDevice for AhciDisk {
    fn block_size(&self) -> usize {
        port::ATA_SECTOR_SIZE
    }

    fn block_count(&self) -> Option<u64> {
        Some(self.sector_count)
    }

    fn read(&mut self, dst: &mut [u8], offset: usize) -> Result<(), BlockDeviceError> {
        if !dst.len().is_multiple_of(port::ATA_SECTOR_SIZE) {
            return Err(BlockDeviceError::UnalignedAccess);
        }

        let sector_count = u64::try_from(dst.len() / port::ATA_SECTOR_SIZE).unwrap();
        let offset = u64::try_from(offset).map_err(|_| BlockDeviceError::OutOfBounds)?;
        if offset.saturating_add(sector_count) > self.sector_count {
            return Err(BlockDeviceError::OutOfBounds);
        }

        self.port
            .read_sectors(offset, dst)
            .map_err(driver_error_to_block)
    }

    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), BlockDeviceError> {
        if !src.len().is_multiple_of(port::ATA_SECTOR_SIZE) {
            return Err(BlockDeviceError::UnalignedAccess);
        }

        let sector_count = u64::try_from(src.len() / port::ATA_SECTOR_SIZE).unwrap();
        let offset = u64::try_from(offset).map_err(|_| BlockDeviceError::OutOfBounds)?;
        if offset.saturating_add(sector_count) > self.sector_count {
            return Err(BlockDeviceError::OutOfBounds);
        }

        self.port
            .write_sectors(offset, src)
            .map_err(driver_error_to_block)
    }
}

const fn driver_error_to_block(error: DriverError) -> BlockDeviceError {
    match error {
        DriverError::Absent | DriverError::Invalid => BlockDeviceError::Unsupported,
        DriverError::Unknown => BlockDeviceError::Io,
    }
}
