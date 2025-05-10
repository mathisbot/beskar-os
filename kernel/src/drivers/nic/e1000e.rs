//! Driver for the Intel e1000e network controller family.
//!
//! See <https://courses.cs.washington.edu/courses/cse451/16au/readings/e1000e.pdf>
//! chapter 10  (p.286) (9 can also be useful) for more information.
//!
//! NB: All registers use host-endianess (LE), except for `ETherType` fiels, which use network-endianess (BE).
use super::Nic;
use crate::{
    drivers::pci::{self, Bar, with_pci_handler},
    locals,
    mem::page_alloc::pmap::PhysicalMapping,
    process,
};
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
use holonet::l2::ethernet::MacAddress;
use hyperdrive::{
    locks::mcs::MUMcsLock,
    ptrs::volatile::{ReadWrite, Volatile},
};

const RX_BUFFERS: usize = 32;
const TX_BUFFERS: usize = 8;

static E1000E: MUMcsLock<E1000e<'static>> = MUMcsLock::uninit();

pub fn init(network_controller: pci::Device) -> DriverResult<()> {
    let Some(Bar::Memory(bar_reg)) =
        with_pci_handler(|handler| handler.read_bar(&network_controller, 0))
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
    let pmap = PhysicalMapping::<M4KiB>::new(reg_paddr, 128 * 1024, flags);
    let reg_vaddr = pmap.translate(reg_paddr).unwrap();

    let (buffer_set, rxdesc_paddr, txdesc_paddr) = BufferSet::new(RX_BUFFERS, TX_BUFFERS);
    let nb_rx = RX_BUFFERS;
    let nb_tx = TX_BUFFERS;

    let mut e1000e = E1000e {
        pci_device: network_controller,
        base: Volatile::new(NonNull::new(reg_vaddr.as_mut_ptr()).unwrap()),
        _physical_mapping: pmap,
        buffer_set,
        rx_curr: 0,
        tx_curr: 0,
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
    base: Volatile<ReadWrite, u32>,
    _physical_mapping: PhysicalMapping<M4KiB>,
    buffer_set: BufferSet<'a>,
    rx_curr: usize,
    tx_curr: usize,
}

impl E1000e<'_> {
    const CTRL: usize = 0x00000; // and 0x00004
    const STATUS: usize = 0x00008;
    const EEC: usize = 0x00010;
    const EERD: usize = 0x00014;
    const CTRLEXT: usize = 0x00018;

    // To enable an interrupt, write 0b1 to the corresponding bit in IMS.
    const IMS: usize = 0x000D0;
    // To disable an interrupt, write 0b1 to the corresponding bit in IMC.
    const IMC: usize = 0x000D8;

    const RCTL: usize = 0x00100;
    const TCTL: usize = 0x00400;
    const TIPG: usize = 0x00410;

    const RDBAL0: usize = 0x02800;
    const RDBAH0: usize = 0x02804;
    const RDLEN: usize = 0x02808;
    const RDH: usize = 0x02810;
    const RDT: usize = 0x02818;

    const TDBAL0: usize = 0x03800;
    const TDBAH0: usize = 0x03804;
    const TDLEN: usize = 0x03808;
    const TDH: usize = 0x03810;
    const TDT: usize = 0x03818;

    const RAL0: usize = 0x05400;
    const RAH0: usize = 0x05404;

    // RCTL

    /// Receiver Enable
    const RCTL_EN: u32 = 1 << 1;
    /// Store Bad Packets
    const RCTL_SBP: u32 = 1 << 2;
    /// Unicast Promiscuous Mode
    const RCTL_UPE: u32 = 1 << 3;
    /// Multicast Promiscuous Mode
    const RCTL_MPE: u32 = 1 << 4;
    /// Long Packet Enable
    const RCTL_LPE: u32 = 1 << 5;
    /// Loopback mode: Normal Operation (default)
    const RCTL_LBM_PHY: u32 = 0b00 << 6;
    /// Loopback mode: MAC Loopback (testing)
    const RCTL_LBM_MAC: u32 = 0b10 << 6;
    const RCTL_RDMTS_HALF: u32 = 0b00 << 8;
    const RCTL_RDMTS_QUARTER: u32 = 0b01 << 8;
    const RCTL_RDMTS_EIGHTH: u32 = 0b10 << 8;
    const RCTL_MO_36: u32 = 0b00 << 12;
    const RCTL_MO_35: u32 = 0b01 << 12;
    const RCTL_MO_34: u32 = 0b10 << 12;
    const RCTL_MO_32: u32 = 0b11 << 12;
    const RCTL_BAM: u32 = 1 << 15;
    const RCTL_BSIZE_256: u32 = (0b11 << 16);
    const RCTL_BSIZE_512: u32 = (0b10 << 16);
    const RCTL_BSIZE_1024: u32 = (0b01 << 16);
    const RCTL_BSIZE_2048: u32 = (0b00 << 16);
    const RCTL_BSIZE_4096: u32 = (0b11 << 16) | (0b1 << 25);
    const RCTL_BSIZE_8192: u32 = (0b10 << 16) | (0b1 << 25);
    const RCTL_BSIZE_16384: u32 = (0b01 << 16) | (0b1 << 25);
    const RCTL_VFE: u32 = 1 << 18;
    const RCTL_CFIEN: u32 = 1 << 19;
    const RCTL_CFI: u32 = 1 << 20;
    const RCTL_DPF: u32 = 1 << 22;
    const RCTL_PMCF: u32 = 1 << 23;
    const RCTL_SECRC: u32 = 1 << 26;

    // TCTL

    const TCTL_EN: u32 = 1 << 1;
    const TCTL_PSP: u32 = 1 << 3;
    const TCTL_CT_SHIFT: u32 = 4;
    const TCTL_COLD_SHIFT: u32 = 12;
    const TCTL_SWXOFF: u32 = 1 << 22;
    const TCTL_RTLC: u32 = 1 << 24;
    const TCTL_UNORTX: u32 = 1 << 25;
    const TCTL_TXDMT_0: u32 = 0b00 << 26;
    const TCTL_TXDMT_1: u32 = 0b01 << 26;
    const TCTL_TXDMT_2: u32 = 0b10 << 26;
    const TCTL_TXDMT_3: u32 = 0b11 << 26;
    const TCTL_RR_NOTHRESH: u32 = 0b11 << 29;

    fn init(&mut self, rxdesc_paddr: PhysAddr, txdesc_paddr: PhysAddr, nb_rx: usize, nb_tx: usize) {
        // Software Initialization Sequence: p.77
        self.reset();
        self.configure_descriptors(rxdesc_paddr, txdesc_paddr, nb_rx, nb_tx);
        self.enable_int();
    }

    fn reset(&self) {
        self.update_reg(Self::CTRL, |ctrl| ctrl | (1 << 26)); // Set RST bit
        while self.read_reg(Self::CTRL) & (1 << 26) != 0 {
            core::hint::spin_loop();
        }
    }

    fn enable_int(&self) {
        let msix = pci::with_pci_handler(|handler| pci::msix::MsiX::new(handler, &self.pci_device));
        let msi = if msix.is_none() {
            pci::with_pci_handler(|handler| pci::msi::Msi::new(handler, &self.pci_device))
        } else {
            None
        };

        let (irq, core_id) = crate::arch::interrupts::new_irq(nic_interrupt_handler, None);

        if let Some(msix) = msix {
            msix.setup_int(irq, 0, core_id);
            pci::with_pci_handler(|handler| msix.enable(handler));
        } else if let Some(msi) = msi {
            pci::with_pci_handler(|handler| {
                msi.setup_int(irq, handler, core_id);
                msi.enable(handler);
            });
        } else {
            unreachable!("No MSI or MSI-X capability found for the network controller.");
        }

        self.write_reg(Self::IMS, 1); // RXDW
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

        self.write_reg(Self::RDBAL0, rx_lo);
        self.write_reg(Self::RDBAH0, rx_hi);
        self.write_reg(
            Self::RDLEN,
            u32::try_from(nb_rx * size_of::<RxDescriptor>()).unwrap(),
        );
        self.write_reg(Self::RDH, 0);
        let rdt_val = u32::from(u16::try_from(nb_rx - 1).unwrap());
        self.write_reg(Self::RDT, rdt_val);
        self.rx_curr = 0;
        let rctl_value = Self::RCTL_EN
            | Self::RCTL_UPE
            | Self::RCTL_MPE
            | Self::RCTL_LBM_PHY
            | Self::RCTL_RDMTS_HALF
            | Self::RCTL_BAM
            | Self::RCTL_BSIZE_4096;
        self.write_reg(Self::RCTL, rctl_value);

        assert!(txdesc_paddr.as_u64().trailing_zeros() >= 4);
        let tx_hi = u32::try_from(rxdesc_paddr.as_u64() >> 32).unwrap();
        let tx_lo = u32::try_from(rxdesc_paddr.as_u64() & u64::from(u32::MAX)).unwrap();

        self.write_reg(Self::TDBAL0, tx_lo);
        self.write_reg(Self::TDBAH0, tx_hi);
        self.write_reg(
            Self::TDLEN,
            u32::try_from(nb_tx * size_of::<TxDescriptor>()).unwrap(),
        );
        self.write_reg(Self::TDH, 0);
        let tdt_val = u32::from(u16::try_from(nb_tx - 1).unwrap());
        self.write_reg(Self::TDT, tdt_val);
        self.tx_curr = 0;
        let tctl_value = Self::TCTL_EN
            | Self::TCTL_PSP
            | (15 << Self::TCTL_CT_SHIFT)
            | (63 << Self::TCTL_COLD_SHIFT)
            | Self::TCTL_RR_NOTHRESH
            | Self::TCTL_TXDMT_0;
        self.write_reg(Self::TCTL, tctl_value);
        self.write_reg(Self::TIPG, 0x0060_2006); // See Section 10.2.6.2 Note
    }

    fn mac_address(&self) -> MacAddress {
        let low = self.read_reg(Self::RAL0);
        let high = self.read_reg(Self::RAH0);

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

    fn write_reg(&self, offset: usize, value: u32) {
        unsafe { self.base.byte_add(offset).write(value) };
    }

    fn update_reg<F>(&self, offset: usize, f: F)
    where
        F: FnOnce(u32) -> u32,
    {
        unsafe { self.base.byte_add(offset).update(f) };
    }
}

extern "x86-interrupt" fn nic_interrupt_handler(_stack_frame: InterruptStackFrame) {
    video::info!("NIC INTERRUPT on core {}", locals!().core_id());
    unsafe { locals!().lapic().force_lock() }.send_eoi();
}

impl Nic for E1000e<'_> {
    fn poll_frame(&self) -> Option<&[u8]> {
        todo!()
    }

    fn send_frame(&self, _frame: &[u8]) {
        todo!()
    }

    fn mac_address(&self) -> MacAddress {
        self.mac_address()
    }
}

#[repr(C, packed)]
struct RxDescriptor {
    buffer_addr: PhysAddr,
    length: u16,
    checksum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

#[repr(C, packed)]
struct TxDescriptor {
    buffer_addr: PhysAddr,
    length: u16,
    cso: u8,
    cmd: u8,
    status: u8,
    css: u8,
    special: u16,
}

struct BufferSet<'a> {
    rx_descriptors: &'a mut [RxDescriptor],
    rx_buffers: Vec<&'a mut [u8]>,
    tx_descriptors: &'a mut [TxDescriptor],
    tx_buffers: Vec<&'a mut [u8]>,
}

impl BufferSet<'_> {
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "Many buffers to allocate")]
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
                        .flush();
                });
            frame
        });
        let rx_descriptors = unsafe {
            core::slice::from_raw_parts_mut(descriptor_page.start_address().as_mut_ptr(), nb_rx)
        };
        let tx_descriptors = unsafe {
            core::slice::from_raw_parts_mut(
                descriptor_page
                    .start_address()
                    .as_mut_ptr::<RxDescriptor>()
                    .add(nb_rx)
                    .cast(),
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
                        page_table.map(page, frame, flags, fralloc).flush();
                    });
                frame
            });

            rx_buffers.push(unsafe {
                core::slice::from_raw_parts_mut(
                    page.start_address().as_mut_ptr(),
                    M4KiB::SIZE.try_into().unwrap(),
                )
            });
            rx_descriptors[i] = RxDescriptor {
                buffer_addr: frame.start_address(),
                length: u16::try_from(M4KiB::SIZE).unwrap(),
                checksum: 0,
                status: 0,
                errors: 0,
                special: 0,
            };
        }

        for (i, page) in page_range.into_iter().skip(nb_rx).take(nb_tx).enumerate() {
            let frame = crate::mem::frame_alloc::with_frame_allocator(|fralloc| {
                let frame = fralloc.alloc::<M4KiB>().unwrap();
                process::current()
                    .address_space()
                    .with_page_table(|page_table| {
                        page_table.map(page, frame, flags, fralloc).flush();
                    });
                frame
            });

            tx_buffers.push(unsafe {
                core::slice::from_raw_parts_mut(
                    page.start_address().as_mut_ptr(),
                    M4KiB::SIZE.try_into().unwrap(),
                )
            });
            tx_descriptors[i] = TxDescriptor {
                buffer_addr: frame.start_address(),
                length: u16::try_from(M4KiB::SIZE).unwrap(),
                cso: 0,
                cmd: 0,
                status: 1 << 0, // TSTA Descriptor Done
                css: 0,
                special: 0,
            };
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
    pub fn tx_buf(&mut self, index: usize) -> &mut [u8] {
        self.tx_buffers[index]
    }
}

impl Drop for BufferSet<'_> {
    fn drop(&mut self) {
        let buffers_start_page =
            Page::<M4KiB>::from_start_address(VirtAddr::from_ptr(self.rx_buffers[0].as_ptr()))
                .unwrap();
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
            Page::<M4KiB>::from_start_address(VirtAddr::from_ptr(self.rx_descriptors.as_ptr()))
                .unwrap();
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
