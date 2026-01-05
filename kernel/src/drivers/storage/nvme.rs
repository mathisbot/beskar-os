//! NVM Express Controller driver, according to
//! <https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-Revision-2.1-2024.08.05-Ratified.pdf>
//! (NVM Express Base Specification Revision 2.1) as well as
//! <https://nvmexpress.org/wp-content/uploads/NVM-Express-PCI-Express-Transport-Specification-Revision-1.1-2024.08.05-Ratified.pdf>
//! (NVMe over PCIe Transport Specification Revision 1.1).
#![expect(clippy::too_long_first_doc_paragraph, reason = "Link references")]

use crate::{
    drivers::pci::MsiHelper,
    locals,
    mem::{frame_alloc, page_alloc::pmap::PhysicalMapping},
};
use ::pci::{Bar, Device, msix::MsiX};
use beskar_core::{
    arch::{
        PhysAddr, VirtAddr,
        paging::{M4KiB, MemSize},
    },
    drivers::{DriverError, DriverResult},
};
use beskar_hal::{paging::page_table::Flags, structures::InterruptStackFrame};
use core::ptr::NonNull;
use hyperdrive::{
    locks::mcs::MUMcsLock,
    ptrs::volatile::{ReadOnly, ReadWrite, Volatile, WriteOnly},
};
use queue::admin::{AdminCompletionQueue, AdminSubmissionQueue};

mod queue;

static NVME_CONTROLLER: MUMcsLock<NvmeControllers> = MUMcsLock::uninit();

const MAX_QUEUES: usize = 3;

pub fn init(nvme: &[Device]) -> DriverResult<()> {
    if nvme.len() > 1 {
        video::warn!("Multiple NVMe controllers found, using the first one");
    }
    let Some(nvme) = nvme.first() else {
        return Err(DriverError::Absent);
    };

    let mut controller = NvmeControllers::new(nvme)?;
    controller.init()?;

    video::info!(
        "NVMe controller initialized with version {}",
        controller.version()
    );

    NVME_CONTROLLER.init(controller);

    Ok(())
}

pub struct NvmeControllers {
    registers_base: VirtAddr,
    msix: MsiX<PhysicalMapping<M4KiB>, MsiHelper>,
    acq: AdminCompletionQueue,
    asq: AdminSubmissionQueue,
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
            panic!("NVMe controller either have no memory BAR or no MSI-X capability");
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

        let asq_doorbell = Volatile::new(
            NonNull::new(unsafe { registers_base.as_mut_ptr::<u16>().byte_add(0x1000) }).unwrap(),
        );
        let acq_doorbell = Volatile::new(
            NonNull::new(unsafe {
                registers_base
                    .as_mut_ptr::<u16>()
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
            max_transfer_sz: 0,
            _pmap: physical_mapping,
        })
    }

    pub fn init(&mut self) -> DriverResult<()> {
        // --- Part One: Controller Bare Initialization ---

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
        self.cc().set_mps(M4KiB::SIZE.try_into().unwrap());

        let css = self.capabilities().css();
        if css & 1 == 1 && css & (1 << 6) == 0 {
            self.cc().set_css(0);
        } else {
            return Err(DriverError::Invalid);
        }

        self.set_asq(self.asq.paddr());
        self.set_acq(self.acq.paddr());

        let max_sz = u16::try_from(M4KiB::SIZE / 64).unwrap() - 1; // 0-based
        self.set_aqa(max_sz, max_sz);

        self.cc().set_iosqes(64);
        self.cc().set_iocqes(16);

        self.cc().enable();
        while !self.csts().ready() {
            if self.csts().fatal() {
                video::warn!("NVMe controller has encountered a fatal error when initializing");
                return Err(DriverError::Unknown);
            }
            core::hint::spin_loop();
        }

        // --- Part Two: Controller Identification ---

        let frame =
            frame_alloc::with_frame_allocator(frame_alloc::FrameAllocator::alloc::<M4KiB>).unwrap();
        let identify_cmd = queue::admin::AdminSubmissionEntry::new_identify(
            queue::admin::IdentifyTarget::Controller,
            frame,
        );
        let identify_cmd_id = identify_cmd.command_id();

        self.asq.push(&identify_cmd);

        let identify_result = {
            let pmap = PhysicalMapping::<M4KiB>::new(
                frame.start_address(),
                size_of::<queue::admin::IdentifyController>(),
                Flags::PRESENT | Flags::NO_EXECUTE | Flags::CACHE_DISABLED,
            )
            .unwrap();
            let vaddr = pmap.translate(frame.start_address()).unwrap();
            let ptr = vaddr.as_ptr::<queue::admin::IdentifyController>();
            // Wait for command completion
            // Completion also triggers an interrupt!
            // TODO: On interrupt, dequeue the completion queue into another Rustier queue/tree
            // intended to be browsed by command identifier
            let res = loop {
                if let Some(v) = self.acq.pop() {
                    break v;
                }
                core::hint::spin_loop();
            };
            assert!(res.is_success());
            assert!(res.command_id() == identify_cmd_id);
            unsafe { ptr.read() }
        };

        self.max_transfer_sz =
            identify_result
                .maximum_data_transfer_size()
                .map_or(u64::MAX, |raw| {
                    let mps_min = u64::from(self.capabilities().mpsmin());
                    mps_min.saturating_mul(1 << raw.get())
                });

        // --- Part Three: I/O queues creation ---

        // TODO: Configure I/O queues

        Ok(())
    }

    pub fn shutdown(&mut self) {
        // TODO: Delete IO queues
        self.cc().disable();
        while self.csts().ready() {
            core::hint::spin_loop();
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
    pub const fn version(&self) -> Version {
        unsafe {
            self.registers_base
                .as_mut_ptr::<Version>()
                .byte_add(0x08)
                .read()
        }
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

extern "x86-interrupt" fn nvme_interrupt_handler(_stack_frame: InterruptStackFrame) {
    video::info!("NVMe INTERRUPT on core {}", locals!().core_id());
    unsafe { locals!().lapic().force_lock() }.send_eoi();
}

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
        u8::try_from((self.read() & (1 << 17)) & 0b11).unwrap()
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
        u8::try_from((self.read() >> 37) & 0xF).unwrap()
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
        u8::try_from((self.read() & (1 << 46)) & 0b11).unwrap()
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
