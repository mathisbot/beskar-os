use crate::{
    drivers::pci::{self, Device},
    locals,
    mem::page_alloc::pmap::PhysicalMapping,
};
use beskar_core::{
    arch::{
        PhysAddr,
        paging::{M4KiB, MemSize as _},
    },
    drivers::{DriverError, DriverResult},
};
use beskar_hal::{paging::page_table::Flags, structures::InterruptStackFrame};
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
mod context;
mod ring;
mod trb;

static XHCI: MUMcsLock<Xhci> = MUMcsLock::uninit();

pub fn init(mut xhci: impl Iterator<Item = (Device, PhysAddr)>) -> DriverResult<()> {
    // TODO: Support multiple xHCI controllers
    let Some((first_xhci_device, first_xhci_paddr)) = xhci.next() else {
        video::warn!("No xHCI controller found");
        return Err(DriverError::Absent);
    };

    let mut xhci = Xhci::new(first_xhci_device, first_xhci_paddr);
    xhci.reinitialize()?;
    video::debug!(
        "xHCI controller with version {} is ready",
        xhci.cap.hci_version()
    );
    XHCI.init(xhci);

    Ok(())
}

/// xHCI Controller
///
/// See <https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/extensible-host-controler-interface-usb-xhci.pdf>
pub struct Xhci {
    /// PCI device for the xHCI controller
    pci_device: Device,
    /// Capabilities registers
    cap: CapabilitiesRegisters,
    /// Operational registers
    op: OperationalRegisters,
    /// Runtime registers
    rt: RuntimeRegisters,
    /// Port registers
    port_regs: PortRegistersSet,
    /// Doorbell registers
    db_regs: DoorbellRegisters,
    /// Command ring
    cmd_ring: Option<ring::CommandRing>,
    /// Event ring
    event_ring: Option<ring::EventRing>,
    /// Device context base address array
    dcbaa: Option<*mut context::DeviceContextBaseAddressArray>,
    /// Physical mapping for the controller registers
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
            cmd_ring: None,
            event_ring: None,
            dcbaa: None,
            _physical_mapping: physical_mapping,
        }
    }

    pub fn reinitialize(&mut self) -> DriverResult<()> {
        // Specification p.69 (sect. 4.2)

        // Reset
        self.op.command().reset();
        while self.op.status().controller_ready() {
            core::hint::spin_loop();
        }
        assert!(!self.op.status().is_running());

        // Controller setup
        let max_slots = self.cap.hcs_params1().max_slots();
        self.op.configure(max_slots, false, false);

        // Initialize the Command Ring
        let cmd_ring = ring::CommandRing::new(200);
        let cmd_ring_phys_addr = cmd_ring.phys_addr();
        self.cmd_ring = Some(cmd_ring);

        // Set the Command Ring Control Register
        // Bits 0-5 are reserved, bit 6 is the Command Ring Running bit (RCS)
        unsafe {
            self.op
                .cmd_ring()
                .write(cmd_ring_phys_addr.as_u64() | (1 << 6));
        }

        // Initialize the Device Context Base Address Array
        let dcbaa_size = usize::from(max_slots) * size_of::<u64>();
        assert!(dcbaa_size <= usize::try_from(M4KiB::SIZE).unwrap());
        let dcbaa_frame = crate::mem::frame_alloc::with_frame_allocator(|frame_allocator| {
            frame_allocator.alloc::<M4KiB>().unwrap()
        });
        let flags = Flags::MMIO_SUITABLE | Flags::WRITABLE;
        let dcbaa_mapping =
            PhysicalMapping::<M4KiB>::new(dcbaa_frame.start_address(), dcbaa_size, flags);
        let dcbaa_phys_addr = dcbaa_mapping.start_frame().start_address();
        let dcbaa_virt_addr = dcbaa_mapping.translate(dcbaa_phys_addr).unwrap();

        // Create the DCBAA
        let dcbaa_ptr = dcbaa_virt_addr.as_mut_ptr();
        unsafe {
            *dcbaa_ptr = context::DeviceContextBaseAddressArray::new(
                core::slice::from_raw_parts_mut(
                    dcbaa_virt_addr.as_mut_ptr(),
                    usize::from(max_slots) + 1,
                ),
                usize::from(max_slots),
            );
        }
        self.dcbaa = Some(dcbaa_ptr);

        // Set the Device Context Base Address Array Pointer Register
        unsafe {
            self.op.dcbaap().write(dcbaa_phys_addr.as_u64());
        }

        // Initialize the Event Ring
        let event_ring = ring::EventRing::new(200);
        let erst_addr = event_ring.segment_table_phys_addr();
        let erst_size = event_ring.segment_table_size();
        self.event_ring = Some(event_ring);

        // Set up the primary interrupter
        let irs = self.rt.irs(0);
        let mut irs_snapshot = irs.read();

        // Set the Event Ring Segment Table Base Address Register
        irs_snapshot.erstba = erst_addr.as_u64();

        // Set the Event Ring Segment Table Size Register
        irs_snapshot.erstsz = u32::try_from(erst_size).unwrap();

        // Set the Event Ring Dequeue Pointer Register
        let dequeue_ptr = self
            .event_ring
            .as_ref()
            .unwrap()
            .current_segment()
            .phys_addr()
            .as_u64();
        irs_snapshot.erdp = dequeue_ptr;

        // Enable interrupts
        irs_snapshot.set_interrupt_enable(true);

        // Write back the interrupter register set
        irs.write(irs_snapshot);

        // Set up MSI-X interrupts
        let Some(msix) =
            pci::with_pci_handler(|handler| pci::msix::MsiX::new(handler, &self.pci_device))
        else {
            return Err(DriverError::Invalid);
        };

        let (irq, core_id) = crate::arch::interrupts::new_irq(xhci_interrupt_handler, None);
        msix.setup_int(irq, 0, core_id);

        pci::with_pci_handler(|handler| {
            msix.enable(handler);
        });

        // Enable interrupts in the command register
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

extern "x86-interrupt" fn xhci_interrupt_handler(_stack_frame: InterruptStackFrame) {
    video::info!("xHCI INTERRUPT on core {}", locals!().core_id());
    handle_xhci_interrupt();
    unsafe { locals!().lapic().force_lock() }.send_eoi();
}

pub fn handle_xhci_interrupt() {
    // with_xhci(|xhci| {
    //     // Check if we have an event ring
    //     let Some(event_ring) = &mut xhci.event_ring else {
    //         return;
    //     };

    //     // Process all events in the event ring
    //     while event_ring.is_current_trb_valid() {
    //         let trb = *event_ring.current_trb();

    //         // Process the event based on its type
    //         match trb.trb_type() {
    //             trb::TrbType::CommandCompletionEvent => {
    //                 let event = trb::CommandCompletionEventTrb::from_trb(&trb);
    //                 video::debug!("Command completion event: {:?}", event.completion_code());
    //             }
    //             trb::TrbType::PortStatusChangeEvent => {
    //                 let event = trb::PortStatusChangeEventTrb::from_trb(&trb);
    //                 let port_id = event.port_id();
    //                 video::debug!("Port status change event for port {}", port_id);

    //                 // Get the port registers
    //                 let port_regs = xhci.port_regs.port_regs(port_id - 1);
    //                 let port_sc = port_regs.port_sc();

    //                 // Check what changed
    //                 if port_sc.connect_status_change() {
    //                     if port_sc.connected() {
    //                         video::debug!("Device connected to port {}", port_id);
    //                         // TODO: Start device enumeration
    //                     } else {
    //                         video::debug!("Device disconnected from port {}", port_id);
    //                         // TODO: Clean up device resources
    //                     }
    //                     port_sc.clear_connect_status_change();
    //                 }

    //                 if port_sc.port_enabled_change() {
    //                     if port_sc.enabled() {
    //                         video::debug!("Port {} enabled", port_id);
    //                     } else {
    //                         video::debug!("Port {} disabled", port_id);
    //                     }
    //                     port_sc.clear_port_enabled_change();
    //                 }

    //                 if port_sc.port_reset_change() {
    //                     video::debug!("Port {} reset complete", port_id);
    //                     port_sc.clear_port_reset_change();
    //                 }
    //             }
    //             _ => {
    //                 video::debug!("Unhandled event type: {:?}", trb.trb_type());
    //             }
    //         }

    //         // Advance to the next event
    //         event_ring.advance();

    //         // Update the Event Ring Dequeue Pointer Register
    //         let irs = xhci.rt.irs(0);
    //         let mut irs_snapshot = irs.read();
    //         let dequeue_ptr = event_ring.current_segment().phys_addr().as_u64()
    //             + (event_ring.dequeue_index() * core::mem::size_of::<trb::Trb>()) as u64;
    //         irs_snapshot.erdp = dequeue_ptr;
    //         irs.write(irs_snapshot);
    //     }
    // });
}

#[inline]
pub fn with_xhci<T, F: FnOnce(&mut Xhci) -> T>(f: F) -> Option<T> {
    XHCI.with_locked_if_init(f)
}
