//! NVM Express Controller driver, according to
//! <https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-Revision-2.1-2024.08.05-Ratified.pdf>
//! (NVM Express Base Specification Revision 2.1) as well as
//! <https://nvmexpress.org/wp-content/uploads/NVM-Express-PCI-Express-Transport-Specification-Revision-1.1-2024.08.05-Ratified.pdf>
//! (NVMe over PCIe Transport Specification Revision 1.1).
#![expect(clippy::too_long_first_doc_paragraph, reason = "Link references")]

use super::super::DmaPage;
use crate::{drivers::pci::MsiHelper, locals, mem::vmm::phys_map::PhysicalMapping};
use ::pci::{Bar, Device, msix::MsiX};
use alloc::vec::Vec;
use beskar_core::{
    arch::{
        PhysAddr, VirtAddr,
        paging::{M4KiB, MemSize as _},
    },
    drivers::{DriverError, DriverResult},
    storage::{BlockDevice, BlockDeviceError},
};
use beskar_hal::{paging::page_table::Flags, structures::InterruptStackFrame};
use core::{num::NonZeroU8, ptr::NonNull};
use driver_shared::mmio::MmioRegister;
use hyperdrive::{
    locks::mcs::MUMcsLock,
    ptrs::volatile::{ReadOnly, ReadWrite, Volatile, WriteOnly},
};
use queue::admin::{AdminCompletionQueue, AdminSubmissionEntry, AdminSubmissionQueue};
use queue::io::{IoCompletionQueue, IoSubmissionEntry, IoSubmissionQueue};

mod queue;

static NVME_CONTROLLER: MUMcsLock<NvmeControllers> = MUMcsLock::uninit();

const MAX_QUEUES: usize = 3;
const IDENTIFY_CONTROLLER_MDTS_OFFSET: usize = 77;
const COMMAND_POLL_LIMIT: usize = 10_000_000;

pub fn init(nvme: &[Device]) -> DriverResult<()> {
    if nvme.len() > 1 {
        crate::warn!("Multiple NVMe controllers found, using the first one");
    }
    let Some(nvme) = nvme.first() else {
        return Err(DriverError::Absent);
    };

    let mut controller = NvmeControllers::new(nvme)?;
    controller.init()?;

    crate::info!(
        "NVMe controller initialized with version {}",
        controller.version()
    );

    let namespaces = controller.discover_namespaces();
    crate::debug!("NVMe controller found {} namespaces", namespaces.len());

    NVME_CONTROLLER.init(controller);

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct NvmeNamespace {
    id: u32,
    block_size: usize,
    block_count: u64,
}

struct NvmeDisk(NvmeNamespace);

impl BlockDevice for NvmeDisk {
    fn block_size(&self) -> usize {
        self.0.block_size
    }

    fn block_count(&self) -> Option<u64> {
        Some(self.0.block_count)
    }

    fn read(&mut self, dst: &mut [u8], offset: usize) -> Result<(), BlockDeviceError> {
        if !dst.len().is_multiple_of(self.0.block_size) {
            return Err(BlockDeviceError::UnalignedAccess);
        }

        let blocks = u64::try_from(dst.len() / self.0.block_size).unwrap();
        let offset = u64::try_from(offset).map_err(|_| BlockDeviceError::OutOfBounds)?;
        if offset.saturating_add(blocks) > self.0.block_count {
            return Err(BlockDeviceError::OutOfBounds);
        }

        with_nvme_controller(|controller| controller.read_namespace(self.0, offset, dst))
            .ok_or(BlockDeviceError::Unsupported)?
            .map_err(driver_error_to_block)
    }

    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), BlockDeviceError> {
        if !src.len().is_multiple_of(self.0.block_size) {
            return Err(BlockDeviceError::UnalignedAccess);
        }

        let blocks = u64::try_from(src.len() / self.0.block_size).unwrap();
        let offset = u64::try_from(offset).map_err(|_| BlockDeviceError::OutOfBounds)?;
        if offset.saturating_add(blocks) > self.0.block_count {
            return Err(BlockDeviceError::OutOfBounds);
        }

        with_nvme_controller(|controller| controller.write_namespace(self.0, offset, src))
            .ok_or(BlockDeviceError::Unsupported)?
            .map_err(driver_error_to_block)
    }
}

const fn driver_error_to_block(error: DriverError) -> BlockDeviceError {
    match error {
        DriverError::Absent | DriverError::Invalid => BlockDeviceError::Unsupported,
        DriverError::Unknown => BlockDeviceError::Io,
    }
}

pub struct NvmeControllers {
    registers_base: VirtAddr,
    msix: MsiX<PhysicalMapping<M4KiB>, MsiHelper>,
    acq: AdminCompletionQueue,
    asq: AdminSubmissionQueue,
    /// IO Completion Queue (QID 1)
    io_cq: Option<IoCompletionQueue>,
    /// IO Submission Queue (QID 1)
    io_sq: Option<IoSubmissionQueue>,
    /// Maximum data transfer size in bytes
    max_transfer_sz: u64,
    _pmap: PhysicalMapping,
}

impl NvmeControllers {
    pub fn new(dev: &Device) -> DriverResult<Self> {
        let (Some(Bar::Memory(bar)), Some(msix)) =
            crate::drivers::pci::with_pci_handler(|handler| {
                (handler.read_bar(dev, 0), MsiX::new(handler, dev))
            })
        else {
            crate::error!("NVMe controller has no memory BAR or no MSI-X capability");
            return Err(DriverError::Absent);
        };

        let paddr = bar.base_address();

        let flags = Flags::MMIO_SUITABLE;

        let doorbell_stride = {
            let physical_mapping =
                PhysicalMapping::<M4KiB>::new(paddr, size_of::<u64>(), flags).unwrap();
            let cap_ptr = NonNull::new(
                physical_mapping
                    .translate(paddr)
                    .unwrap()
                    .as_mut_ptr::<u64>(),
            )
            .unwrap();
            let cap = Capabilities(Volatile::new(cap_ptr));
            cap.dstrd()
        };

        let physical_mapping = PhysicalMapping::<M4KiB>::new(
            paddr,
            0x1000 + 2 * (MAX_QUEUES + 1) * doorbell_stride,
            flags,
        )
        .unwrap();
        let registers_base = physical_mapping.translate(paddr).unwrap();

        let asq_doorbell = MmioRegister::new(
            NonNull::new(unsafe { registers_base.as_mut_ptr::<u32>().byte_add(0x1000) }).unwrap(),
        );
        let acq_doorbell = MmioRegister::new(
            NonNull::new(unsafe {
                registers_base
                    .as_mut_ptr::<u32>()
                    .byte_add(0x1000 + doorbell_stride)
            })
            .unwrap(),
        );
        let submission_queue = queue::admin::AdminSubmissionQueue::new(asq_doorbell)?;
        let completion_queue = queue::admin::AdminCompletionQueue::new(acq_doorbell)?;

        Ok(Self {
            registers_base,
            msix,
            acq: completion_queue,
            asq: submission_queue,
            io_cq: None,
            io_sq: None,
            max_transfer_sz: 0,
            _pmap: physical_mapping,
        })
    }

    pub fn init(&mut self) -> DriverResult<()> {
        // Controller Bare Initialization

        self.cc().disable();
        while self.csts().ready() {
            core::hint::spin_loop();
        }

        let (irq, core_id) = crate::arch::interrupts::new_irq(nvme_interrupt_handler, None);

        self.msix.setup_int(irq, 0, core_id);
        crate::drivers::pci::with_pci_handler(|handler| self.msix.enable(handler));

        if self.capabilities().mpsmin() > u32::try_from(M4KiB::SIZE).unwrap() {
            return Err(DriverError::Invalid);
        }
        if self.capabilities().mpsmax() < u32::try_from(M4KiB::SIZE).unwrap() {
            return Err(DriverError::Invalid);
        }
        self.cc().set_mps(M4KiB::SIZE.try_into().unwrap());

        let css = self.capabilities().css();
        if css & 1 != 0 {
            // NVM command set supported; select it
            self.cc().set_css(0);
        } else {
            return Err(DriverError::Invalid);
        }

        self.set_asq(self.asq.paddr());
        self.set_acq(self.acq.paddr());

        let asqs = u16::try_from(M4KiB::SIZE / 64).unwrap() - 1; // 0-based
        let acqs = u16::try_from(M4KiB::SIZE / 16).unwrap() - 1; // 0-based
        self.set_aqa(acqs, asqs);

        self.cc().set_iosqes(64);
        self.cc().set_iocqes(16);

        self.cc().enable();
        while !self.csts().ready() {
            if self.csts().fatal() {
                crate::warn!("NVMe controller has encountered a fatal error when initializing");
                return Err(DriverError::Unknown);
            }
            core::hint::spin_loop();
        }

        // Controller Identification

        let identify_page = DmaPage::new(usize::try_from(M4KiB::SIZE).unwrap())?;
        let identify_cmd = AdminSubmissionEntry::new_identify(
            queue::admin::IdentifyTarget::Controller,
            identify_page.frame(),
        );

        let identify_res = self.submit_synchronous_admin(&identify_cmd);
        let maximum_data_transfer_size = match identify_res {
            Ok(_) => unsafe {
                identify_page
                    .as_ptr::<u8>()
                    .byte_add(IDENTIFY_CONTROLLER_MDTS_OFFSET)
                    .read()
            },
            Err(err) => {
                crate::error!("Identify Controller command failed");
                return Err(err);
            }
        };

        self.max_transfer_sz = NonZeroU8::new(maximum_data_transfer_size).map_or(u64::MAX, |raw| {
            let mps_min = u64::from(self.capabilities().mpsmin());
            1_u64
                .checked_shl(u32::from(raw.get()))
                .map_or(u64::MAX, |factor| mps_min.saturating_mul(factor))
        });

        // I/O queues creation

        let dstrd = self.capabilities().dstrd();
        let io_sq_doorbell = MmioRegister::new(
            NonNull::new(unsafe {
                self.registers_base
                    .as_mut_ptr::<u32>()
                    .byte_add(0x1000 + 2 * dstrd)
            })
            .unwrap(),
        );
        let io_cq_doorbell = MmioRegister::new(
            NonNull::new(unsafe {
                self.registers_base
                    .as_mut_ptr::<u32>()
                    .byte_add(0x1000 + 3 * dstrd)
            })
            .unwrap(),
        );

        let io_cq = IoCompletionQueue::new(io_cq_doorbell)?;
        let io_sq = IoSubmissionQueue::new(io_sq_doorbell)?;

        // Respect MQES limit (value is 0-based in CAP, so +1 entries)
        let max_entries = self.capabilities().mqes().saturating_add(1);
        let cq_entries = core::cmp::min(io_cq.entries(), max_entries).saturating_sub(1);
        let sq_entries = core::cmp::min(io_sq.entries(), max_entries).saturating_sub(1);

        // Create IO Completion Queue (QID 1), interrupt enabled on vector 0
        let create_cq =
            AdminSubmissionEntry::new_create_io_cq(1, cq_entries, io_cq.paddr(), 0, true);
        if self.submit_synchronous_admin(&create_cq).is_err() {
            crate::error!("IO CQ command failed");
            return Err(DriverError::Unknown);
        }

        // Create IO Submission Queue (QID 1) targeting CQID 1, priority 0
        let create_sq = AdminSubmissionEntry::new_create_io_sq(1, sq_entries, io_sq.paddr(), 1, 0);
        if self.submit_synchronous_admin(&create_sq).is_err() {
            crate::error!("IO SQ command failed");
            return Err(DriverError::Unknown);
        }

        self.io_cq = Some(io_cq);
        self.io_sq = Some(io_sq);

        crate::debug!(
            "NVMe IO queues created: SQ entries={}, CQ entries={}",
            sq_entries,
            cq_entries
        );

        Ok(())
    }

    pub fn shutdown(&mut self) {
        // TODO: Delete IO queues via admin delete commands
        // TODO: Wait for all pending IO commands to complete
        self.cc().disable();
        while self.csts().ready() {
            core::hint::spin_loop();
        }
    }

    fn discover_namespaces(&mut self) -> Vec<NvmeNamespace> {
        let mut namespaces = Vec::new();

        let Ok(page) = DmaPage::new(usize::try_from(M4KiB::SIZE).unwrap()) else {
            crate::warn!("Failed to allocate NVMe namespace list buffer");
            return namespaces;
        };

        let identify_res = self.identify(queue::admin::IdentifyTarget::NamespaceList, &page);
        if identify_res.is_ok() {
            let ids = unsafe { core::slice::from_raw_parts(page.as_ptr::<u32>(), 1024) };
            let namespace_ids: Vec<u32> = ids.iter().copied().take_while(|id| *id != 0).collect();

            for nsid in namespace_ids {
                match self.identify_namespace(nsid) {
                    Ok(namespace) => {
                        crate::debug!(
                            "Registering NVMe namespace {} as block device ({} blocks, {} bytes/block)",
                            namespace.id,
                            namespace.block_count,
                            namespace.block_size
                        );
                        namespaces.push(namespace);
                    }
                    Err(err) => {
                        crate::warn!("Failed to identify NVMe namespace {}: {:?}", nsid, err);
                    }
                }
            }
        } else {
            crate::warn!("Failed to identify NVMe namespace list");
        }

        namespaces
    }

    fn identify_namespace(&mut self, nsid: u32) -> DriverResult<NvmeNamespace> {
        let page = DmaPage::new(usize::try_from(M4KiB::SIZE).unwrap())?;
        self.identify(queue::admin::IdentifyTarget::Namespace(nsid), &page)?;

        let data = page.as_bytes();
        let block_count = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let flbas = data[26] & 0x0F;
        let lbaf_offset = 128 + usize::from(flbas) * 4;
        let lbads = data[lbaf_offset + 2];
        let block_size = 1_usize
            .checked_shl(u32::from(lbads))
            .ok_or(DriverError::Invalid)?;

        if block_count == 0 || block_size == 0 || block_size > usize::try_from(M4KiB::SIZE).unwrap()
        {
            return Err(DriverError::Invalid);
        }

        Ok(NvmeNamespace {
            id: nsid,
            block_size,
            block_count,
        })
    }

    fn identify(
        &mut self,
        target: queue::admin::IdentifyTarget,
        page: &DmaPage,
    ) -> DriverResult<()> {
        let identify_cmd = queue::admin::AdminSubmissionEntry::new_identify(target, page.frame());
        let res = self.submit_synchronous_admin(&identify_cmd);

        if let Err(err) = res {
            crate::error!("NVMe Identify command failed");
            return Err(err);
        }

        Ok(())
    }

    fn read_namespace(
        &mut self,
        namespace: NvmeNamespace,
        offset: u64,
        dst: &mut [u8],
    ) -> DriverResult<()> {
        let mut offset = offset;
        let max_blocks = usize::try_from(M4KiB::SIZE).unwrap() / namespace.block_size;
        if max_blocks == 0 {
            return Err(DriverError::Invalid);
        }

        for chunk in dst.chunks_mut(max_blocks * namespace.block_size) {
            let page = DmaPage::new(usize::try_from(M4KiB::SIZE).unwrap())?;
            let blocks = u16::try_from(chunk.len() / namespace.block_size)
                .map_err(|_| DriverError::Invalid)?;
            let command =
                IoSubmissionEntry::new_read(namespace.id, offset, blocks, page.phys_addr());
            let result = self.submit_synchronous_io(&command);
            if result.is_ok() {
                page.copy_to_slice(chunk);
            }
            result?;
            offset += u64::from(blocks);
        }

        Ok(())
    }

    fn write_namespace(
        &mut self,
        namespace: NvmeNamespace,
        offset: u64,
        src: &[u8],
    ) -> DriverResult<()> {
        let mut offset = offset;
        let max_blocks = usize::try_from(M4KiB::SIZE).unwrap() / namespace.block_size;
        if max_blocks == 0 {
            return Err(DriverError::Invalid);
        }

        for chunk in src.chunks(max_blocks * namespace.block_size) {
            let page = DmaPage::new(usize::try_from(M4KiB::SIZE).unwrap())?;
            page.copy_from_slice(chunk);

            let blocks = u16::try_from(chunk.len() / namespace.block_size)
                .map_err(|_| DriverError::Invalid)?;
            let command =
                IoSubmissionEntry::new_write(namespace.id, offset, blocks, page.phys_addr());
            let result = self.submit_synchronous_io(&command);
            result?;
            offset += u64::from(blocks);
        }

        Ok(())
    }

    fn submit_synchronous_io(&mut self, command: &IoSubmissionEntry) -> DriverResult<()> {
        let command_id = command.command_id();
        let Some(sq) = self.io_sq.as_mut() else {
            return Err(DriverError::Invalid);
        };
        sq.push(command);

        let Some(cq) = self.io_cq.as_mut() else {
            return Err(DriverError::Invalid);
        };
        let Some(res) = wait_for_completion(cq, command_id) else {
            crate::warn!("NVMe I/O command {} timed out", command_id);
            return Err(DriverError::Unknown);
        };

        if !res.is_success() {
            crate::error!("NVMe I/O command failed: status={:04x}", res.status_code());
            return Err(DriverError::Unknown);
        }

        Ok(())
    }

    fn submit_synchronous_admin(
        &mut self,
        command: &AdminSubmissionEntry,
    ) -> Result<queue::admin::AdminCompletionEntry, DriverError> {
        let command_id = command.command_id();
        self.asq.push(command);

        let Some(completion) = wait_for_completion(&mut self.acq, command_id) else {
            crate::warn!("NVMe admin command {} timed out", command_id);
            return Err(DriverError::Unknown);
        };

        if completion.is_success() {
            Ok(completion)
        } else {
            Err(DriverError::Unknown)
        }
    }

    #[must_use]
    #[inline]
    pub const fn capabilities(&self) -> Capabilities {
        let ptr = NonNull::new(self.registers_base.as_mut_ptr()).unwrap();
        Capabilities(Volatile::new(ptr))
    }

    #[must_use]
    #[inline]
    pub fn version(&self) -> Version {
        let raw = unsafe {
            self.registers_base
                .as_ptr::<u32>()
                .byte_add(0x08)
                .read_volatile()
        };
        Version::from_raw(raw)
    }

    #[must_use]
    #[inline]
    /// When using MSI-X, the interrupt mask table defined as part of MSI-X should be used to
    /// mask interrupts. Host software shall not access this property when configured for MSI-X.
    pub const fn intms(&self) -> Volatile<WriteOnly, u32> {
        let ptr = unsafe { self.registers_base.as_mut_ptr::<u32>().byte_add(0x0C) };
        Volatile::new(NonNull::new(ptr).unwrap())
    }

    #[must_use]
    #[inline]
    /// When using MSI-X, the interrupt mask table defined as part of MSI-X should be used to
    /// unmask interrupts. Host software shall not access this property when configured for MSI-X.
    pub const fn intmc(&self) -> Volatile<WriteOnly, u32> {
        let ptr = unsafe { self.registers_base.as_mut_ptr::<u32>().byte_add(0x10) };
        Volatile::new(NonNull::new(ptr).unwrap())
    }

    #[must_use]
    #[inline]
    /// This property modifies settings for the controller.
    pub const fn cc(&self) -> Configuration {
        let ptr = unsafe { self.registers_base.as_mut_ptr::<u32>().byte_add(0x14) };
        Configuration(Volatile::new(NonNull::new(ptr).unwrap()))
    }

    #[must_use]
    #[inline]
    /// This property is used to read the controller status.
    pub const fn csts(&self) -> Status {
        let ptr = unsafe { self.registers_base.as_mut_ptr::<u32>().byte_add(0x1C) };
        Status(Volatile::new(NonNull::new(ptr).unwrap()))
    }

    fn set_aqa(&self, acqs: u16, asqs: u16) {
        assert!(acqs <= 0xFFF);
        assert!(asqs <= 0xFFF);
        let value = u32::from(asqs) | (u32::from(acqs) << 16);
        let ptr = unsafe { self.registers_base.as_mut_ptr::<u32>().byte_add(0x24) };
        unsafe { ptr.write_volatile(value) };
    }

    fn set_asq(&self, addr: PhysAddr) {
        let ptr = unsafe { self.registers_base.as_mut_ptr::<u64>().byte_add(0x28) };
        unsafe { ptr.write_volatile(addr.as_u64() & !0xFFF) };
    }

    fn set_acq(&self, addr: PhysAddr) {
        let ptr = unsafe { self.registers_base.as_mut_ptr::<u64>().byte_add(0x30) };
        unsafe { ptr.write_volatile(addr.as_u64() & !0xFFF) };
    }
}

trait CompletionQueueLike {
    type Entry;

    fn pop_completion(&mut self) -> Option<Self::Entry>;
}

trait CompletionEntryLike: Copy {
    fn command_id(self) -> u16;
}

impl CompletionQueueLike for IoCompletionQueue {
    type Entry = queue::io::IoCompletionEntry;

    #[inline]
    fn pop_completion(&mut self) -> Option<Self::Entry> {
        self.pop()
    }
}

impl CompletionEntryLike for queue::io::IoCompletionEntry {
    #[inline]
    fn command_id(self) -> u16 {
        self.command_id()
    }
}

impl CompletionQueueLike for AdminCompletionQueue {
    type Entry = queue::admin::AdminCompletionEntry;

    #[inline]
    fn pop_completion(&mut self) -> Option<Self::Entry> {
        self.pop()
    }
}

impl CompletionEntryLike for queue::admin::AdminCompletionEntry {
    #[inline]
    fn command_id(self) -> u16 {
        self.command_id()
    }
}

fn wait_for_completion<Q>(queue: &mut Q, command_id: u16) -> Option<Q::Entry>
where
    Q: CompletionQueueLike,
    Q::Entry: CompletionEntryLike,
{
    let mut remaining = COMMAND_POLL_LIMIT;
    while remaining != 0 {
        if let Some(v) = queue.pop_completion()
            && v.command_id() == command_id
        {
            return Some(v);
        }
        remaining -= 1;
        core::hint::spin_loop();
    }

    None
}

extern "C" fn nvme_interrupt_handler_inner(_stack_frame: &InterruptStackFrame) {
    crate::debug!("NVMe INTERRUPT on core {}", locals!().core_id());
    unsafe { locals!().lapic().force_lock() }.send_eoi();
}
beskar_hal::isr!(nvme_interrupt_handler, nvme_interrupt_handler_inner);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Version {
    tertiary: u8,
    minor: u8,
    major: u16,
}

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NVMe {}.{}.{}", self.major, self.minor, self.tertiary)
    }
}

impl Version {
    #[must_use]
    #[inline]
    const fn from_raw(raw: u32) -> Self {
        Self {
            tertiary: (raw & 0xFF) as u8,
            minor: ((raw >> 8) & 0xFF) as u8,
            major: (raw >> 16) as u16,
        }
    }
}

pub struct Capabilities(Volatile<ReadOnly, u64>);

impl Capabilities {
    #[must_use]
    #[inline]
    fn read(&self) -> u64 {
        unsafe { self.0.read() }
    }

    #[must_use]
    #[inline]
    /// Maximum Queue Entries Supported
    pub fn mqes(&self) -> u16 {
        u16::try_from(self.read() & 0xFFFF).unwrap()
    }

    #[must_use]
    #[inline]
    /// Contiguous Queues Required
    pub fn cqr(&self) -> bool {
        (self.read() & (1 << 16)) != 0
    }

    #[must_use]
    #[inline]
    /// Arbitration Mechanism Support
    ///
    /// Bit 0: Weighted Round Robin with Urgent Priority Class
    /// Bit 1: Vendor Specific
    /// Bits 2-7: Always 0
    pub fn ams(&self) -> u8 {
        u8::try_from((self.read() >> 17) & 0b11).unwrap()
    }

    #[must_use]
    #[inline]
    /// Worst case time for the controller to be ready
    ///
    /// This field is in 500ms units, for a maximum value of 127.5 seconds.
    pub fn to(&self) -> u8 {
        u8::try_from((self.read() >> 24) & 0xFF).unwrap()
    }

    #[must_use]
    #[inline]
    /// Doorbell stride
    ///
    /// Each Submission Queue and Completion Queue Doorbell register is 32-bits in size.
    /// This register indicates the stride between doorbell registers.
    pub fn dstrd(&self) -> usize {
        let power = (self.read() >> 32) & 0xF;
        1 << (power + 2)
    }

    #[must_use]
    #[inline]
    /// NVM Subsystem Reset Support
    pub fn nssrs(&self) -> bool {
        (self.read() & (1 << 36)) != 0
    }

    #[must_use]
    #[inline]
    /// Command Sets Supported
    ///
    /// Bit 0: NVM command set
    /// Bits 1-5: Reserved
    /// Bit 6: IO command set
    /// Bit 7: No IO command set
    pub fn css(&self) -> u8 {
        u8::try_from((self.read() >> 37) & 0xFF).unwrap()
    }

    #[must_use]
    #[inline]
    /// Boot Partition Support
    pub fn bps(&self) -> bool {
        (self.read() & (1 << 45)) != 0
    }

    #[must_use]
    #[inline]
    /// Controller Power Scope
    ///
    /// 0b00: Unknown
    /// 0b01: Controller scope
    /// 0b10: Domain scope
    /// 0b11: NVM subsystem scope
    pub fn cps(&self) -> u8 {
        u8::try_from((self.read() >> 46) & 0b11).unwrap()
    }

    #[must_use]
    #[inline]
    /// Minimum host memory page size that the controller supports.
    pub fn mpsmin(&self) -> u32 {
        let power = (self.read() >> 48) & 0xF;
        1 << (power + 12)
    }

    #[must_use]
    #[inline]
    /// Maximum host memory page size that the controller supports.
    pub fn mpsmax(&self) -> u32 {
        let power = (self.read() >> 52) & 0xF;
        1 << (power + 12)
    }
}

/// Controller Configuration
///
/// Fields specified page 79 of the specification
pub struct Configuration(Volatile<ReadWrite, u32>);

impl Configuration {
    #[must_use]
    #[inline]
    fn read(&self) -> u32 {
        unsafe { self.0.read() }
    }

    #[inline]
    fn write(&self, value: u32) {
        unsafe { self.0.write(value) }
    }

    #[inline]
    /// Enable the controller
    pub fn enable(&self) {
        self.write(self.read() | 1);
    }

    #[inline]
    /// Disable the controller
    pub fn disable(&self) {
        self.write(self.read() & !1);
    }

    #[inline]
    /// Set the IO Submission Queue Entry Size
    fn set_iosqes(&self, iosqes: u16) {
        const IOSQES_MASK: u32 = 0xF << 16;

        assert!(iosqes.is_power_of_two());
        let iosqes = iosqes.trailing_zeros();
        assert!(iosqes <= 0xF);

        self.write((self.read() & !IOSQES_MASK) | ((iosqes << 16) & IOSQES_MASK));
    }

    #[inline]
    /// Set the IO Completion Queue Entry Size
    fn set_iocqes(&self, iocqes: u16) {
        const IOCQES_MASK: u32 = 0xF << 20;

        assert!(iocqes.is_power_of_two());
        let iocqes = iocqes.trailing_zeros();
        assert!(iocqes <= 0xF);

        self.write((self.read() & !IOCQES_MASK) | ((iocqes << 20) & IOCQES_MASK));
    }

    /// Set the Memory Page Size
    pub fn set_mps(&self, mps: u32) {
        const MPS_MASK: u32 = 0xF << 7;

        assert!(mps.is_power_of_two());
        assert!(mps >= 4096);
        let mps = mps.trailing_zeros() - 12;
        assert!(mps <= 0xF);

        self.write((self.read() & !MPS_MASK) | ((mps << 7) & MPS_MASK));
    }

    /// Set the Command Set Selected
    fn set_css(&self, value: u8) {
        const CSS_MASK: u32 = 0x7 << 4;
        assert!(value <= 0x7);
        self.write((self.read() & !CSS_MASK) | ((u32::from(value) << 4) & CSS_MASK));
    }

    // TODO: Implement the rest of the fields
}

pub struct Status(Volatile<ReadOnly, u32>);

impl Status {
    #[must_use]
    #[inline]
    fn read(&self) -> u32 {
        unsafe { self.0.read() }
    }

    #[must_use]
    #[inline]
    /// Controller Ready
    pub fn ready(&self) -> bool {
        (self.read() & 1) != 0
    }

    #[must_use]
    #[inline]
    /// Controller has encountered a fatal error
    pub fn fatal(&self) -> bool {
        (self.read() & (1 << 1)) != 0
    }

    // TODO: Implement the rest of the fields
}

#[inline]
pub fn with_nvme_controller<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut NvmeControllers) -> R,
{
    NVME_CONTROLLER.with_locked_if_init(f)
}
