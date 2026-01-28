use crate::mem::page_alloc::pmap::PhysicalMapping;
use ::pci::{Bar, Device};
use beskar_core::{
    arch::{VirtAddr, paging::M4KiB},
    drivers::{DriverError, DriverResult},
};
use beskar_hal::paging::page_table::Flags;

mod command;
mod fis;
pub use fis::{AtaCommand, FisD2H, FisH2D, FisType};
mod port;
use port::AhciPort;
mod registers;
use registers::AhciRegisters;

/// Timeout for controller operations (in iterations)
const CONTROLLER_TIMEOUT: usize = 100_000_000;

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
    let pmap =
        PhysicalMapping::<M4KiB>::new(ahci_paddr, 256, flags).map_err(|_| DriverError::Unknown)?;

    let ahci_base = pmap.translate(ahci_paddr).ok_or(DriverError::Unknown)?;

    let mut ahci = Ahci::new(ahci_base, pmap);
    ahci.initialize()?;

    video::info!("AHCI controller initialized successfully");

    Ok(())
}

/// AHCI controller instance
pub struct Ahci {
    base: VirtAddr,
    pmap: PhysicalMapping,
    /// Number of ports supported by this controller
    port_count: u32,
}

impl Ahci {
    /// Create a new AHCI controller instance
    fn new(base: VirtAddr, pmap: PhysicalMapping) -> Self {
        let regs = unsafe { AhciRegisters::from_base(base) };

        let version = regs.version();
        let capabilities = regs.capabilities();
        let port_count = capabilities.np() + 1;

        video::debug!("AHCI version: {}.{}", version >> 16, version & 0xFFFF);
        video::debug!("AHCI supports {} ports", port_count);

        Self {
            base,
            pmap,
            port_count,
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
            video::warn!("AHCI mode enable timeout");
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
    fn probe_ports(&self) -> DriverResult<()> {
        let regs = unsafe { AhciRegisters::from_base(self.base) };
        let ports_implemented = regs.ports_implemented();

        for port_idx in 0..self.port_count {
            // Check if this port is implemented
            if (ports_implemented & (1 << port_idx)) == 0 {
                continue;
            }

            let port_offset = 0x100 + (u64::from(port_idx) * 0x80);
            let port_addr = self.base + port_offset;
            let port = AhciPort::new(port_addr, port_idx);

            // Check if a device is present
            if port.is_device_present() {
                port.initialize()?;
                video::debug!("AHCI port {} initialized with device", port_idx);
            }
        }

        Ok(())
    }
}
