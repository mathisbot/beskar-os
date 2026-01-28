//! Driver for the Intel e1000e network controller family.
//!
//! See <https://courses.cs.washington.edu/courses/cse451/16au/readings/e1000e.pdf>
//! chapter 10  (p.286) (9 can also be useful) for more information.
//!
//! NB: All registers use host-endianess (LE), except for `ETherType` fields, which use network-endianess (BE).

mod descriptors;
mod registers;

use self::{
    descriptors::{RxDescriptor, TxDescriptor},
    registers::{CtrlFlags, IntFlags, RctlFlags, Registers, TctlFlags},
};
use super::Nic;
use crate::{drivers::pci::MsiHelper, locals, mem::page_alloc::pmap::PhysicalMapping, process};
use ::pci::Bar;
use alloc::vec::Vec;
use beskar_core::{
    arch::{
        PhysAddr, VirtAddr,
        paging::{CacheFlush as _, M4KiB, Mapper, MemSize as _, Page},
    },
    drivers::{DriverError, DriverResult},
};
use beskar_hal::{paging::page_table::Flags, structures::InterruptStackFrame};
use core::ptr::NonNull;
use driver_shared::mmio::MmioRegister;
use holonet::l2::ethernet::MacAddress;
use hyperdrive::{locks::mcs::MUMcsLock, ptrs::volatile::ReadWrite};

const RX_BUFFERS: usize = 32;
const TX_BUFFERS: usize = 8;

static E1000E: MUMcsLock<E1000e<'static>> = MUMcsLock::uninit();

pub fn init(network_controller: pci::Device) -> DriverResult<()> {
    let Some(Bar::Memory(bar_reg)) =
        crate::drivers::pci::with_pci_handler(|handler| handler.read_bar(&network_controller, 0))
    else {
        // FIXME: Apparently, some network controllers use IO BARs
        video::warn!("Network controller does not have a memory BAR");
        return Err(DriverError::Absent);
    };

    let reg_paddr = bar_reg.base_address();

    // Section 10.1.1.2 (p.286): Flash memory address is in BAR1.
    // Section 10.1.1.5 (p.287): If needed, IO memory is in BAR2.
    // Section 10.1.1.3 (p.286): MSI-X tables are in BAR3.

    let flags = Flags::MMIO_SUITABLE;
    // Max size is 128 KiB
    let pmap = PhysicalMapping::<M4KiB>::new(reg_paddr, 128 * 1024, flags).unwrap();
    let reg_vaddr = pmap.translate(reg_paddr).unwrap();

    let (buffer_set, rxdesc_paddr, txdesc_paddr) = BufferSet::new(RX_BUFFERS, TX_BUFFERS);
    let nb_rx = RX_BUFFERS;
    let nb_tx = TX_BUFFERS;

    let mut e1000e = E1000e {
        pci_device: network_controller,
        base: MmioRegister::new(NonNull::new(reg_vaddr.as_mut_ptr()).unwrap()),
        _physical_mapping: pmap,
        buffer_set,
        rx_curr: core::cell::Cell::new(0),
        tx_curr: core::cell::Cell::new(0),
    };
    e1000e.init(rxdesc_paddr, txdesc_paddr, nb_rx, nb_tx);

    video::info!(
        "Intel e1000e network controller initialized. MAC: {}",
        e1000e.mac_address()
    );

    E1000E.init(e1000e);

    Ok(())
}

pub struct E1000e<'a> {
    pci_device: pci::Device,
    base: MmioRegister<ReadWrite, u32>,
    _physical_mapping: PhysicalMapping<M4KiB>,
    buffer_set: BufferSet<'a>,
    rx_curr: core::cell::Cell<usize>,
    tx_curr: core::cell::Cell<usize>,
}

impl E1000e<'_> {
    fn init(&mut self, rxdesc_paddr: PhysAddr, txdesc_paddr: PhysAddr, nb_rx: usize, nb_tx: usize) {
        // Software Initialization Sequence: p.77
        self.reset();
        self.configure_descriptors(rxdesc_paddr, txdesc_paddr, nb_rx, nb_tx);
        self.enable_int();
    }

    fn reset(&mut self) {
        self.update_reg(Registers::CTRL, |ctrl| ctrl | CtrlFlags::RST);
        while self.read_reg(Registers::CTRL) & CtrlFlags::RST != 0 {
            core::hint::spin_loop();
        }
    }

    fn enable_int(&mut self) {
        let msix = crate::drivers::pci::with_pci_handler(|handler| {
            pci::msix::MsiX::<PhysicalMapping<M4KiB>, MsiHelper>::new(handler, &self.pci_device)
        });
        let msi = if msix.is_none() {
            crate::drivers::pci::with_pci_handler(|handler| {
                pci::msi::Msi::<MsiHelper>::new(handler, &self.pci_device)
            })
        } else {
            None
        };

        let (irq, core_id) = crate::arch::interrupts::new_irq(nic_interrupt_handler, None);

        if let Some(msix) = msix {
            msix.setup_int(irq, 0, core_id);
            crate::drivers::pci::with_pci_handler(|handler| msix.enable(handler));
        } else if let Some(msi) = msi {
            crate::drivers::pci::with_pci_handler(|handler| {
                msi.setup_int(irq, handler, core_id);
                msi.enable(handler);
            });
        } else {
            unreachable!("No MSI or MSI-X capability found for the network controller.");
        }

        self.write_reg(
            Registers::IMS,
            IntFlags::RXT0 | IntFlags::RXDMT0 | IntFlags::TXDW | IntFlags::LSC,
        );
    }

    fn configure_descriptors(
        &mut self,
        rxdesc_paddr: PhysAddr,
        txdesc_paddr: PhysAddr,
        nb_rx: usize,
        nb_tx: usize,
    ) {
        assert!(rxdesc_paddr.as_u64().trailing_zeros() >= 4);
        let rx_hi = u32::try_from(rxdesc_paddr.as_u64() >> 32).unwrap();
        let rx_lo = u32::try_from(rxdesc_paddr.as_u64() & u64::from(u32::MAX)).unwrap();

        self.write_reg(Registers::RDBAL0, rx_lo);
        self.write_reg(Registers::RDBAH0, rx_hi);
        self.write_reg(
            Registers::RDLEN,
            u32::try_from(nb_rx * size_of::<RxDescriptor>()).unwrap(),
        );
        self.write_reg(Registers::RDH, 0);
        let rdt_val = u32::from(u16::try_from(nb_rx - 1).unwrap());
        self.write_reg(Registers::RDT, rdt_val);
        self.rx_curr.set(0);
        let rctl_value = RctlFlags::EN
            | RctlFlags::UPE
            | RctlFlags::MPE
            | RctlFlags::LBM_PHY
            | RctlFlags::RDMTS_HALF
            | RctlFlags::BAM
            | RctlFlags::BSIZE_4096;
        self.write_reg(Registers::RCTL, rctl_value);

        assert!(txdesc_paddr.as_u64().trailing_zeros() >= 4);
        let tx_hi = u32::try_from(txdesc_paddr.as_u64() >> 32).unwrap();
        let tx_lo = u32::try_from(txdesc_paddr.as_u64() & u64::from(u32::MAX)).unwrap();

        self.write_reg(Registers::TDBAL0, tx_lo);
        self.write_reg(Registers::TDBAH0, tx_hi);
        self.write_reg(
            Registers::TDLEN,
            u32::try_from(nb_tx * size_of::<TxDescriptor>()).unwrap(),
        );
        self.write_reg(Registers::TDH, 0);
        let tdt_val = u32::from(u16::try_from(nb_tx - 1).unwrap());
        self.write_reg(Registers::TDT, tdt_val);
        self.tx_curr.set(0);
        let tctl_value = TctlFlags::EN
            | TctlFlags::PSP
            | (15 << TctlFlags::CT_SHIFT)
            | (63 << TctlFlags::COLD_SHIFT)
            | TctlFlags::RR_NOTHRESH
            | TctlFlags::TXDMT_0;
        self.write_reg(Registers::TCTL, tctl_value);
        self.write_reg(Registers::TIPG, 0x0060_2006); // See Section 10.2.6.2 Note
    }

    fn mac_address(&self) -> MacAddress {
        let low = self.read_reg(Registers::RAL0);
        let high = self.read_reg(Registers::RAH0);

        assert_eq!((high >> 16) & 0b11, 0, "MAC address is not DEST.");
        assert_eq!(high >> 31, 1, "MAC address is not valid.");

        MacAddress::new([
            u8::try_from(low & 0xFF).unwrap(),
            u8::try_from((low >> 8) & 0xFF).unwrap(),
            u8::try_from((low >> 16) & 0xFF).unwrap(),
            u8::try_from((low >> 24) & 0xFF).unwrap(),
            u8::try_from(high & 0xFF).unwrap(),
            u8::try_from((high >> 8) & 0xFF).unwrap(),
        ])
    }

    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { self.base.byte_add(offset).read() }
    }

    fn write_reg(&mut self, offset: usize, value: u32) {
        unsafe { self.base.byte_add(offset).write(value) };
    }

    fn update_reg<F>(&mut self, offset: usize, f: F)
    where
        F: FnOnce(u32) -> u32,
    {
        unsafe { self.base.byte_add(offset).update(f) };
    }

    fn advance_rx(&mut self) {
        let rx_idx = self.rx_curr.get();

        // Reset the descriptor for reuse
        let desc = self.buffer_set.rx_desc_mut(rx_idx);
        desc.reset();

        // Advance to next descriptor
        let next = (rx_idx + 1) % RX_BUFFERS;
        self.rx_curr.set(next);

        // Update RDT register to notify hardware
        self.write_reg(Registers::RDT, u32::try_from(next).unwrap());
    }
}

extern "x86-interrupt" fn nic_interrupt_handler(_stack_frame: InterruptStackFrame) {
    E1000E.with_locked(|e1000e| {
        // Read and acknowledge interrupt cause
        let icr = e1000e.read_reg(Registers::ICR);

        if icr & IntFlags::RXT0 != 0 || icr & IntFlags::RXDMT0 != 0 {
            // TODO: Packet received (notify network stack)
        }

        if icr & IntFlags::TXDW != 0 {
            // TODO: Transmit done
        }

        if icr & IntFlags::LSC != 0 {
            // Link status changed
            let status = e1000e.read_reg(Registers::STATUS);
            let link_up = (status & 0x02) != 0;
            if link_up {
                video::debug!("Network link is up");
            } else {
                video::debug!("Network link is down");
            }
        }
    });

    unsafe { locals!().lapic().force_lock() }.send_eoi();
}

impl Nic for E1000e<'_> {
    fn poll_frame(&self) -> Option<&[u8]> {
        let rx_idx = self.rx_curr.get();
        let desc = self.buffer_set.rx_desc(rx_idx);

        if !desc.is_done() || !desc.is_end_of_packet() {
            return None;
        }

        let packet_len = desc.packet_length() as usize;
        if packet_len == 0 || packet_len > 4096 {
            // Invalid packet length - will be skipped on consume_frame()
            return None;
        }

        // Check for errors in the packet
        if desc.has_errors() {
            // Packet has errors - will be skipped on consume_frame()
            return None;
        }

        Some(&self.buffer_set.rx_buf(rx_idx)[..packet_len])
    }

    fn consume_frame(&mut self) {
        self.advance_rx();
    }

    fn send_frame(&mut self, frame: &[u8]) {
        if frame.is_empty() || frame.len() > 4096 {
            video::warn!("Invalid frame size: {}", frame.len());
            return;
        }

        let tx_idx = self.tx_curr.get();

        // Wait for a free TX descriptor
        let desc = self.buffer_set.tx_desc(tx_idx);
        if !desc.is_done() {
            video::warn!("No free TX descriptor available");
            return;
        }

        // Copy frame to TX buffer
        let tx_buf = self.buffer_set.tx_buf_mut(tx_idx);
        tx_buf[..frame.len()].copy_from_slice(frame);

        // Prepare descriptor for transmission
        let desc = self.buffer_set.tx_desc_mut(tx_idx);
        desc.prepare_for_send(u16::try_from(frame.len()).unwrap());

        // Advance to next descriptor
        let next = (tx_idx + 1) % TX_BUFFERS;
        self.tx_curr.set(next);

        // Update TDT register to notify hardware
        self.write_reg(Registers::TDT, u32::try_from(next).unwrap());
    }

    fn mac_address(&self) -> MacAddress {
        self.mac_address()
    }
}

struct BufferSet<'a> {
    rx_descriptors: &'a mut [RxDescriptor],
    rx_buffers: Vec<&'a mut [u8]>,
    tx_descriptors: &'a mut [TxDescriptor],
    tx_buffers: Vec<&'a mut [u8]>,
}

impl BufferSet<'_> {
    #[must_use]
    pub fn new(nb_rx: usize, nb_tx: usize) -> (Self, PhysAddr, PhysAddr) {
        assert!(
            nb_rx * size_of::<RxDescriptor>() + nb_tx * size_of::<TxDescriptor>()
                < M4KiB::SIZE.try_into().unwrap()
        );

        let mut rx_buffers = Vec::with_capacity(nb_rx);
        let mut tx_buffers = Vec::with_capacity(nb_tx);

        // The NIC will use physical address to access buffers.
        // Thus, we must make sure that everything is physically contiguous.
        // The easiest way I found to do this is to allocate a full frame for each object.
        // It gives a very nice 4096 bytes buffer, which is common.

        let descriptor_page = process::current()
            .address_space()
            .with_pgalloc(|palloc| palloc.allocate_pages(1).unwrap())
            .start();
        let flags = Flags::MMIO_SUITABLE;
        let descriptor_frame = crate::mem::frame_alloc::with_frame_allocator(|fralloc| {
            let frame = fralloc.alloc::<M4KiB>().unwrap();
            process::current()
                .address_space()
                .with_page_table(|page_table| {
                    page_table
                        .map(descriptor_page, frame, flags, fralloc)
                        .unwrap()
                        .flush();
                });
            frame
        });

        // SAFETY: We just allocated and mapped this page. The memory is valid and properly aligned.
        // The lifetime 'a is tied to BufferSet, ensuring these slices don't outlive the allocation.
        let rx_descriptors = unsafe {
            core::slice::from_raw_parts_mut(
                descriptor_page.start_address().as_mut_ptr::<RxDescriptor>(),
                nb_rx,
            )
        };

        // SAFETY: Same page as rx_descriptors, offset by nb_rx descriptors.
        // The size check at the beginning ensures this doesn't overflow the page.
        let tx_descriptors = unsafe {
            core::slice::from_raw_parts_mut(
                descriptor_page
                    .start_address()
                    .as_mut_ptr::<RxDescriptor>()
                    .add(nb_rx)
                    .cast::<TxDescriptor>(),
                nb_tx,
            )
        };

        let page_range = process::current()
            .address_space()
            .with_pgalloc(|palloc| palloc.allocate_pages(nb_rx as u64 + nb_tx as u64).unwrap());

        for (i, page) in page_range.into_iter().take(nb_rx).enumerate() {
            let frame = crate::mem::frame_alloc::with_frame_allocator(|fralloc| {
                let frame = fralloc.alloc::<M4KiB>().unwrap();
                process::current()
                    .address_space()
                    .with_page_table(|page_table| {
                        page_table.map(page, frame, flags, fralloc).unwrap().flush();
                    });
                frame
            });

            // SAFETY: We just allocated and mapped this page. The memory is valid.
            // The lifetime 'a ensures this slice doesn't outlive the BufferSet.
            rx_buffers.push(unsafe {
                core::slice::from_raw_parts_mut(
                    page.start_address().as_mut_ptr(),
                    M4KiB::SIZE.try_into().unwrap(),
                )
            });
            rx_descriptors[i] =
                RxDescriptor::new(frame.start_address(), u16::try_from(M4KiB::SIZE).unwrap());
        }

        for (i, page) in page_range.into_iter().skip(nb_rx).take(nb_tx).enumerate() {
            let frame = crate::mem::frame_alloc::with_frame_allocator(|fralloc| {
                let frame = fralloc.alloc::<M4KiB>().unwrap();
                process::current()
                    .address_space()
                    .with_page_table(|page_table| {
                        page_table.map(page, frame, flags, fralloc).unwrap().flush();
                    });
                frame
            });

            // SAFETY: We just allocated and mapped this page. The memory is valid.
            // The lifetime 'a ensures this slice doesn't outlive the BufferSet.
            tx_buffers.push(unsafe {
                core::slice::from_raw_parts_mut(
                    page.start_address().as_mut_ptr(),
                    M4KiB::SIZE.try_into().unwrap(),
                )
            });
            tx_descriptors[i] =
                TxDescriptor::new(frame.start_address(), u16::try_from(M4KiB::SIZE).unwrap());
        }

        assert_eq!(rx_buffers.len(), nb_rx);

        (
            Self {
                rx_descriptors,
                rx_buffers,
                tx_descriptors,
                tx_buffers,
            },
            descriptor_frame.start_address(),
            descriptor_frame.start_address()
                + u64::try_from(nb_rx * size_of::<RxDescriptor>()).unwrap(),
        )
    }

    #[must_use]
    #[inline]
    pub fn rx_desc(&self, index: usize) -> &RxDescriptor {
        &self.rx_descriptors[index]
    }

    #[must_use]
    #[inline]
    pub fn rx_buf(&self, index: usize) -> &[u8] {
        self.rx_buffers[index]
    }

    #[must_use]
    #[inline]
    pub fn tx_desc(&self, index: usize) -> &TxDescriptor {
        &self.tx_descriptors[index]
    }

    #[must_use]
    #[inline]
    pub fn rx_desc_mut(&mut self, index: usize) -> &mut RxDescriptor {
        &mut self.rx_descriptors[index]
    }

    #[must_use]
    #[inline]
    pub fn tx_desc_mut(&mut self, index: usize) -> &mut TxDescriptor {
        &mut self.tx_descriptors[index]
    }

    #[must_use]
    #[inline]
    pub fn tx_buf_mut(&mut self, index: usize) -> &mut [u8] {
        self.tx_buffers[index]
    }
}

impl Drop for BufferSet<'_> {
    fn drop(&mut self) {
        let buffers_start_page = VirtAddr::from_ptr(self.rx_buffers[0].as_ptr()).page::<M4KiB>();
        let buffer_page_range = Page::range_inclusive(
            buffers_start_page,
            buffers_start_page
                + (self.rx_buffers.len() + self.tx_buffers.len() - 1)
                    .try_into()
                    .unwrap(),
        );

        for page in buffer_page_range {
            let frame = process::current().address_space().with_page_table(|pt| {
                let (frame, tlb) = pt.unmap(page).unwrap();
                tlb.flush();
                frame
            });
            crate::mem::frame_alloc::with_frame_allocator(|fralloc| fralloc.free(frame));
        }
        process::current()
            .address_space()
            .with_pgalloc(|palloc| palloc.free_pages(buffer_page_range));

        let descriptors_page =
            Page::<M4KiB>::containing_address(VirtAddr::from_ptr(self.rx_descriptors.as_ptr()));
        let descriptors_frame = process::current().address_space().with_page_table(|pt| {
            let (frame, tlb) = pt.unmap(descriptors_page).unwrap();
            tlb.flush();
            frame
        });
        crate::mem::frame_alloc::with_frame_allocator(|fralloc| fralloc.free(descriptors_frame));
        process::current().address_space().with_pgalloc(|palloc| {
            palloc.free_pages(Page::range_inclusive(descriptors_page, descriptors_page));
        });
    }
}

pub fn with_e1000e<F, R>(f: F) -> R
where
    F: FnOnce(&mut E1000e) -> R,
{
    E1000E.with_locked(f)
}

pub fn e1000e_available() -> bool {
    E1000E.is_initialized()
}
