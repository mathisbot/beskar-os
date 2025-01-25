// TODO: Remove
#![allow(dead_code)]

use beskar_core::arch::commons::{PhysAddr, paging::M4KiB};

use crate::mem::page_alloc::pmap::{self, PhysicalMapping};
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
use db::DoorbellRegistersSet;

static XHCI: MUMcsLock<Xhci> = MUMcsLock::uninit();

pub fn init(mut xhci_paddrs: impl Iterator<Item = PhysAddr>) {
    // TODO: Support multiple xHCI controllers
    let Some(first_xhci_paddr) = xhci_paddrs.next() else {
        crate::warn!("No xHCI controller found");
        return;
    };

    let xhci = Xhci::new(first_xhci_paddr);
    xhci.reinitialize();
    XHCI.init(xhci);
}

/// See <https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/extensible-host-controler-interface-usb-xhci.pdf>
pub struct Xhci {
    cap: CapabilitiesRegisters,
    op: OperationalRegisters,
    rt: RuntimeRegisters,
    port_regs: PortRegistersSet,
    db_regs: DoorbellRegistersSet,
    _physical_mapping: PhysicalMapping,
}

impl Xhci {
    const PORT_REG_OFFSET: u64 = 0x400;

    #[must_use]
    pub fn new(paddr: PhysAddr) -> Self {
        let flags = pmap::FLAGS_MMIO;

        // At first, we only map enough memory to read the capabilities register
        let physical_mapping =
            PhysicalMapping::<M4KiB>::new(paddr, CapabilitiesRegisters::MIN_LENGTH, flags);
        let vaddr = physical_mapping.translate(paddr).unwrap();

        let cap = CapabilitiesRegisters::new(vaddr);
        let cap_length = usize::from(cap.cap_length());
        let max_ports = cap.hcs_params1().max_ports();
        let max_slots = cap.hcs_params1().max_slots();
        let rtsoff = usize::try_from(cap.rtsoff()).unwrap();
        let dboff = usize::try_from(cap.dboff()).unwrap();

        let _ = (vaddr, cap); // We are about to unmap the memory, so it's best to shadow related variables

        // We can now map more memory to read the operational registers
        let total_length = (usize::try_from(Self::PORT_REG_OFFSET).unwrap()
            + size_of::<u32>() * usize::from(max_ports)) // PR
        .max(rtsoff + size_of::<u32>()) // RT - TODO: RT regs have more registers
        .max(dboff + size_of::<u32>() * usize::from(max_slots)); // DB
        let physical_mapping = PhysicalMapping::new(paddr, total_length, flags);

        let reg_base_vaddr = physical_mapping.translate(paddr).unwrap();

        let cap = CapabilitiesRegisters::new(reg_base_vaddr);
        let op = OperationalRegisters::new(reg_base_vaddr + u64::try_from(cap_length).unwrap());
        let rt = RuntimeRegisters::new(reg_base_vaddr + u64::try_from(rtsoff).unwrap());
        let port_regs = PortRegistersSet::new(reg_base_vaddr + Self::PORT_REG_OFFSET, max_ports);
        let db_regs =
            DoorbellRegistersSet::new(reg_base_vaddr + u64::try_from(dboff).unwrap(), max_slots);

        Self {
            cap,
            op,
            rt,
            port_regs,
            db_regs,
            _physical_mapping: physical_mapping,
        }
    }

    pub fn reinitialize(&self) {
        self.op.command().reset();
        self.op.command().run_stop(true);
        assert!(self.op.status().is_running());
        self.op.command().set_interrupts(true);
        crate::debug!(
            "xHCI controller with version {} is ready",
            self.cap.hci_version()
        );
    }
}

#[inline]
pub fn with_xhci<T, F: FnOnce(&mut Xhci) -> T>(f: F) -> Option<T> {
    XHCI.try_with_locked(f)
}
