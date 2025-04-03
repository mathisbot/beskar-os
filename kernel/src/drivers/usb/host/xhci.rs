use crate::{
    drivers::pci::{self, Device},
    mem::page_alloc::pmap::PhysicalMapping,
};
use beskar_core::{
    arch::commons::{
        PhysAddr,
        paging::{Flags, M4KiB},
    },
    drivers::{DriverError, DriverResult},
};
use hyperdrive::locks::mcs::MUMcsLock;

mod cap;
use cap::CapabilitiesRegisters;
mod op;
use op::OperationalRegisters;
mod reg;
use reg::PortRegistersSet;
mod rt;
use rt::RuntimeRegisters;
mod db;
use db::DoorbellRegisters;

static XHCI: MUMcsLock<Xhci> = MUMcsLock::uninit();

pub fn init(mut xhci: impl Iterator<Item = (Device, PhysAddr)>) -> DriverResult<()> {
    // TODO: Support multiple xHCI controllers
    let Some((first_xhci_device, first_xhci_paddr)) = xhci.next() else {
        crate::warn!("No xHCI controller found");
        return Err(DriverError::Absent);
    };

    let xhci = Xhci::new(first_xhci_device, first_xhci_paddr);
    xhci.reinitialize()?;
    crate::debug!(
        "xHCI controller with version {} is ready",
        xhci.cap.hci_version()
    );
    XHCI.init(xhci);

    Ok(())
}

/// See <https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/extensible-host-controler-interface-usb-xhci.pdf>
pub struct Xhci {
    pci_device: Device,
    cap: CapabilitiesRegisters,
    op: OperationalRegisters,
    rt: RuntimeRegisters,
    port_regs: PortRegistersSet,
    db_regs: DoorbellRegisters,
    _physical_mapping: PhysicalMapping,
}

impl Xhci {
    const PORT_REG_OFFSET: u64 = 0x400;

    #[must_use]
    pub fn new(device: Device, paddr: PhysAddr) -> Self {
        let flags = Flags::MMIO_SUITABLE;

        // At first, we only map enough memory to read the capabilities register
        let physical_mapping =
            PhysicalMapping::<M4KiB>::new(paddr, CapabilitiesRegisters::MIN_LENGTH, flags);
        let vaddr = physical_mapping.translate(paddr).unwrap();

        let cap = CapabilitiesRegisters::new(vaddr);
        let cap_length = cap.cap_length();
        let max_ports = cap.hcs_params1().max_ports();
        let max_slots = cap.hcs_params1().max_slots();
        let rtsoff = usize::try_from(cap.rtsoff()).unwrap();
        let dboff = usize::try_from(cap.dboff()).unwrap();

        let _ = (vaddr, cap); // We are about to unmap the memory, so it's best to shadow related variables

        // We can now map more memory to access the rest of the registers
        let total_length = dboff + size_of::<u32>() * usize::from(max_slots); // DB registers are at the end of the memory
        let physical_mapping = PhysicalMapping::new(paddr, total_length, flags);

        let reg_base_vaddr = physical_mapping.translate(paddr).unwrap();

        let cap = CapabilitiesRegisters::new(reg_base_vaddr);
        let op = OperationalRegisters::new(reg_base_vaddr + u64::from(cap_length));
        let rt = RuntimeRegisters::new(reg_base_vaddr + u64::try_from(rtsoff).unwrap());
        let port_regs = PortRegistersSet::new(reg_base_vaddr + Self::PORT_REG_OFFSET, max_ports);
        let db_regs =
            DoorbellRegisters::new(reg_base_vaddr + u64::try_from(dboff).unwrap(), max_slots);

        Self {
            pci_device: device,
            cap,
            op,
            rt,
            port_regs,
            db_regs,
            _physical_mapping: physical_mapping,
        }
    }

    pub fn reinitialize(&self) -> DriverResult<()> {
        // Specification p.69 (sect. 4.2)

        // Reset
        self.op.command().reset();
        while self.op.status().controller_ready() {
            core::hint::spin_loop();
        }
        assert!(!self.op.status().is_running());

        // Controller setup
        // TODO: Use more, up to self.cap.hcs_params1().max_slots())
        self.op.configure(1, false, false);

        // TODO: Program DCBAAP
        // TODO: Define the Command Ring Dequeue Pointer

        let Some(msix) =
            pci::with_pci_handler(|handler| pci::msix::MsiX::new(handler, &self.pci_device))
        else {
            return Err(DriverError::Invalid);
        };
        msix.setup_int(crate::arch::interrupts::Irq::Xhci, 0);

        // TODO: Init Runtime Registers (ER: p.179 sect. 4.9.4)

        pci::with_pci_handler(|handler| {
            msix.enable(handler);
        });

        self.op.command().set_interrupts(true);

        // Enable the controller
        self.op.command().run_stop(true);
        if self.op.status().is_running() {
            Ok(())
        } else {
            Err(DriverError::Unknown)
        }
    }
}

pub fn handle_xhci_interrupt() {
    // TODO: Handle xHCI interrupts
}

#[inline]
pub fn with_xhci<T, F: FnOnce(&mut Xhci) -> T>(f: F) -> Option<T> {
    XHCI.with_locked_if_init(f)
}
