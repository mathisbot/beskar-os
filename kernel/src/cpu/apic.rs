//! Advanced Programmable Interrupt Controller (APIC) driver.

use core::{
    ptr::NonNull,
    sync::atomic::{AtomicU8, Ordering},
};

use hyperdrive::volatile::{Access, Volatile};
use timer::LapicTimer;
use x86_64::{
    instructions::port::Port,
    structures::paging::{Mapper, PageSize, PageTableFlags, PhysFrame, Size4KiB},
    PhysAddr,
};

use crate::{
    cpu::cpuid,
    locals,
    mem::{frame_alloc, page_alloc, page_table},
};

use super::interrupts::Irq;

pub mod ap;
pub mod ipi;
pub mod timer;

#[must_use]
pub fn apic_id() -> u8 {
    let cpuid_res = cpuid::cpuid(1);
    u8::try_from((cpuid_res.ebx >> 24) & 0xFF).unwrap()
}

/// Initializes the Local APIC.
///
/// This function must be called on each core.
pub fn init_lapic() {
    let x2apic_supported = cpuid::check_feature(cpuid::CpuFeature::X2APIC);
    if locals!().core_id() == 0 && !x2apic_supported {
        log::warn!("x2APIC not supported");
    }

    let lapic_paddr = crate::boot::acpi::ACPI
        .get()
        .map_or_else(LocalApic::get_paddr_from_msr, |acpi| {
            acpi.madt().lapic_paddr()
        });

    ensure_pic_disabled();

    let mut lapic = LocalApic::from_paddr(lapic_paddr);

    lapic.timer().calibrate();

    locals!().lapic().init(lapic);
}

/// Initializes the IO APICs.
///
/// This function must only be called once by the BSP.
pub fn init_ioapic() {
    if let Some(acpi) = crate::boot::acpi::ACPI.get() {
        for io_apic in acpi.madt().io_apics() {
            let io_apic = IoApic::new(io_apic.addr(), io_apic.gsi_base());
            io_apic.init();
        }
    }
}

/// Enables/disables interrupts.
///
/// ## Panics
///
/// This function will panic if the APIC is not enabled.
fn enable_disable_interrupts(enable: bool) {
    locals!().lapic().try_with_locked(|lapic| {
        unsafe {
            lapic.base.byte_add(0xF0).update(|value| {
                if enable {
                    // Enable spurious interrupt
                    value | 0x100
                } else {
                    // Disable spurious interrupt
                    value & !0x100
                }
            });
        };
    });
}

pub struct LocalApic {
    base: Volatile<u32>,
    timer: LapicTimer,
}

impl LocalApic {
    #[must_use]
    fn get_paddr_from_msr() -> PhysAddr {
        let msr = x86_64::registers::model_specific::Msr::new(0x1B);
        let base = unsafe { msr.read() };

        assert!((base >> 11) & 1 == 1, "APIC not enabled");

        PhysAddr::new(base & 0xF_FFFF_F000)
    }

    #[must_use]
    pub fn from_paddr(paddr: PhysAddr) -> Self {
        let frame = PhysFrame::<Size4KiB>::from_start_address(paddr).unwrap();

        let apic_flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE
            | PageTableFlags::NO_CACHE;

        let page = page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.allocate_pages::<Size4KiB>(1).unwrap().start
        });

        frame_alloc::with_frame_allocator(|frame_allocator| {
            page_table::with_page_table(|page_table| {
                unsafe { page_table.map_to(page, frame, apic_flags, &mut *frame_allocator) }
                    .unwrap()
                    .flush();
            });
        });

        // Register spurious interrupt handler
        let base_ptr: *mut u32 = page.start_address().as_mut_ptr();
        let apic_spurious = unsafe { &mut *base_ptr.byte_add(0xF0) };
        *apic_spurious &= !0xFF; // Clear spurious handler index
        *apic_spurious |= u32::from(super::interrupts::Irq::Spurious as u8); // Set spurious handler index
        *apic_spurious |= 0x100; // Enable spurious interrupt

        let base = Volatile::new(NonNull::new(base_ptr).unwrap(), Access::ReadWrite);

        Self {
            base,
            timer: timer::LapicTimer::new(timer::Configuration::new(base, Irq::Timer)),
        }
    }

    pub fn send_ipi(&self, ipi: &ipi::Ipi) {
        let icr_low = unsafe { self.base.byte_add(0x300) };
        let icr_high = unsafe { self.base.byte_add(0x310) };
        // Safety:
        // The ICR registers are read/write and their addresses are valid.
        unsafe { ipi.send(icr_low, icr_high) };
    }

    #[must_use]
    #[inline]
    pub const fn timer(&mut self) -> &mut timer::LapicTimer {
        &mut self.timer
    }

    pub fn send_eoi(&mut self) {
        unsafe { self.base.byte_add(0xB0).write(0) };
    }
}

/// Ensures that PIC 8259 is disabled.
///
/// This a mandatory step before enabling the APIC.
fn ensure_pic_disabled() {
    unsafe {
        let mut cmd1 = Port::<u8>::new(0x20);
        let mut data1 = Port::<u8>::new(0x21);

        let mut cmd2 = Port::<u8>::new(0xA0);
        let mut data2 = Port::<u8>::new(0xA1);

        let mut fence = Port::<u8>::new(0x80);

        // Reinitialize the PIC controllers
        cmd1.write(0x11);
        cmd2.write(0x11);
        fence.write(0);

        // Set the new IRQ offsets to match with APIC IRQs
        data1.write(0xF8);
        data2.write(0xFF);
        fence.write(0);

        // Tell the PICs that they're chained
        data1.write(0x04);
        fence.write(0);
        data2.write(0x02);
        fence.write(0);

        // Set PICs to x86 mode
        data1.write(0x01);
        data2.write(0x01);
        fence.write(0);

        // Disable all IRQs
        data1.write(0xFF);
        data2.write(0xFF);
    };
}

static IOAPICID_CNTER: AtomicU8 = AtomicU8::new(0);

/// I/O APIC
///
/// See <https://pdos.csail.mit.edu/6.828/2016/readings/ia32/ioapic.pdf>
pub struct IoApic {
    base: Volatile<u32>,
    gsi_base: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IoApicReg {
    Id,
    Version,
    Arbitration,
    /// Index must be between 0 and 23 (inclusive)
    ///
    /// These registers are 64-bit, but must be accessed as two 32-bit registers
    /// (obviously). They in fact have 2 indices :
    /// `self.index()` and `self.index() + 1`.
    Redirection(u8),
}

impl IoApicReg {
    #[must_use]
    fn index(self) -> u32 {
        match self {
            Self::Id => 0,
            Self::Version => 1,
            Self::Arbitration => 2,
            Self::Redirection(idx) => {
                assert!(idx <= 23, "Redirection index must be less than 24");
                // These registers are 64-bit
                0x10 + 2 * u32::from(idx)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// Physical destination
    ///
    /// Number describes the APIC ID of the destination
    /// and must be 4 bits long.
    ///
    /// Yes, this means that only 16 processors can be addressed.
    Physical(u8), // TODO: I think x2APIC uses 32 bits
    /// Logical destination
    ///
    /// Number describes a set of processors
    /// (specify the logical destination address)
    Logical(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerMode {
    Edge = 0,
    Level = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinPolarity {
    High = 0,
    Low = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Fixed = 0b000,
    LowestPriority = 0b001,
    Smi = 0b010,
    // Reserved = 0b011,
    Nmi = 0b100,
    Init = 0b101,
    // Reserved = 0b110,
    ExtInt = 0b111,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Redirection {
    destination: Destination,
    /// Allows to mask the interrupt (except for NMIs !!!)
    interrupt_mask: bool,
    trigger_mode: TriggerMode,
    // This bit is used for level triggered interrupts. Its meaning is undefined for
    // edge triggered interrupts. For level triggered interrupts, this bit is set to 1 when local APIC(s)
    // accept the level interrupt sent by the IOAPIC. The Remote IRR bit is set to 0 when an EOI
    // message with a matching interrupt vector is received from a local APIC.
    remote_irr: bool,
    pin_polarity: PinPolarity,
    delivery_mode: DeliveryMode,
    /// The value of the interrupt vector must be
    /// between 0x10 and 0xFE (inclusive)
    int_vec: u8,
}

impl IoApic {
    #[must_use]
    pub fn new(base: PhysAddr, gsi_base: u32) -> Self {
        let frame = PhysFrame::<Size4KiB>::containing_address(base);

        let frame_end_addr = frame.start_address() + (Size4KiB::SIZE - 1);
        assert!(
            base + u64::try_from(size_of::<u64>()).unwrap() <= frame_end_addr,
            "IOAPIC frame must not cross a 4KiB boundary"
        );

        let apic_flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE
            | PageTableFlags::NO_CACHE;

        // FIXME: I don't quite like that each IOAPIC gets its own page
        // Apparently, IOAPICs only live in Physical 0xFEC0..00, so one page per 16 IOAPICs?
        // Or maybe keep track of mapped pages and check if the page is already mapped?
        let page = page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.allocate_pages::<Size4KiB>(1)
        })
        .unwrap()
        .start;

        frame_alloc::with_frame_allocator(|frame_allocator| {
            page_table::with_page_table(|page_table| {
                // Safety:
                // The frame is reserved by the UEFI, so it is already allocated.
                unsafe { page_table.map_to(page, frame, apic_flags, frame_allocator) }
                    .unwrap()
                    .flush();
            });
        });

        let base = Volatile::new(
            NonNull::new((page.start_address() + (base - frame.start_address())).as_mut_ptr())
                .unwrap(),
            Access::ReadWrite,
        );

        Self { base, gsi_base }
    }

    pub fn init(&self) {
        enable_disable_interrupts(false);

        let id_offset = IOAPICID_CNTER.fetch_add(1, Ordering::Relaxed);
        let cpu_count = crate::locals::get_ready_core_count();
        // Each APIC device must have a unique ID to be uniquely addressed
        // on the APIC Bus.
        self.set_id(cpu_count + id_offset);

        // TODO: Setup redirection entries (See MADT)
        let isos = crate::boot::acpi::ACPI.get().unwrap().madt().io_iso();
        for iso in isos {
            // TODO: Implement Redirection
            // let red = Redirection {
            //     // ???
            // }
            // self.set_redirection(iso.gsi(), iso.redirection());
            log::warn!("Unhandled IOAPIC redirection: {:?}", iso);
        }

        enable_disable_interrupts(true);
    }
}

// Safe register access
impl IoApic {
    #[must_use]
    /// Returns the ID of the IO APIC.
    ///
    /// This ID is NOT valid until the IO APIC has been initialized.
    pub fn id(&self) -> u8 {
        let id = self.read_reg(IoApicReg::Id);
        u8::try_from((id >> 24) & 0xF).unwrap()
    }

    fn set_id(&self, id: u8) {
        assert!(id < 0xF, "IOAPIC ID must be less than 0xF");
        // Safety:
        // IOAPICID is read/write
        unsafe { self.update_reg(IoApicReg::Id, u32::from(id), 4, 24) };
    }

    #[must_use]
    pub fn version(&self) -> u8 {
        let ver = self.read_reg(IoApicReg::Version);
        u8::try_from(ver & 0xFF).unwrap()
    }

    #[must_use]
    pub fn max_red_ent(&self) -> u8 {
        let ver = self.read_reg(IoApicReg::Version);
        u8::try_from((ver >> 16) & 0xFF).unwrap()
    }

    #[must_use]
    pub fn arbitration_id(&self) -> u8 {
        let arb = self.read_reg(IoApicReg::Arbitration);
        u8::try_from((arb >> 24) & 0xF).unwrap()
    }

    pub fn set_redirection(&self, index: u8, redirection: Redirection) {
        assert!(
            index < self.max_red_ent(),
            "Redirection index must be less than the max redirection entries"
        );

        let nmi_sources = crate::boot::acpi::ACPI
            .get()
            .unwrap()
            .madt()
            .io_nmi_sources();
        for _nmi_source in nmi_sources {
            // TODO: Check if NMI source
            // If NMI source, don't forget to use nmi_source.flags() to set the flags
            // and to switch delivery mode to NMI
        }

        let reg = IoApicReg::Redirection(index);

        let low_idx = reg.index();
        let high_idx = low_idx + 1;

        // High register

        let mut high_value = 0;
        // Destination
        high_value |= match redirection.destination {
            Destination::Physical(id) => {
                assert!(id < 0xF, "Physical destination ID must be less than 0xF");
                u32::from(id)
            }
            Destination::Logical(id) => u32::from(id),
        };
        unsafe { self.update_reg_idx(high_idx, high_value, 8, 24) };

        // Low register

        let mut low_value = 0;
        low_value |= u32::from(redirection.interrupt_mask) << 16;
        low_value |= (redirection.trigger_mode as u32) << 15;
        low_value |=
            u32::from(redirection.remote_irr && redirection.trigger_mode == TriggerMode::Level)
                << 14;
        low_value |= (redirection.pin_polarity as u32) << 13;
        low_value |= match redirection.destination {
            Destination::Physical(_) => 0,
            Destination::Logical(_) => 1,
        } << 11;
        low_value |= (redirection.delivery_mode as u32) << 8;
        low_value |= u32::from(redirection.int_vec);

        // Note that bit 12 is read only and we are still writing to it.
        // This is because writes to this bit are ignored.
        unsafe { self.update_reg_idx(low_idx, low_value, 17, 0) };
    }
}

// Raw register access
impl IoApic {
    /// Updates the value of a register.
    ///
    /// Specifically, it will update bits \[idx..idx+len\[ of the register `reg`
    /// with bits \[0..len\[ of `value`.
    ///
    /// # Safety
    ///
    /// The index must be a valid writable register index.
    unsafe fn update_reg(&self, reg: IoApicReg, value: u32, len: u8, bit: u8) {
        unsafe { self.update_reg_idx(reg.index(), value, len, bit) };
    }

    unsafe fn update_reg_idx(&self, idx: u32, value: u32, len: u8, bit: u8) {
        let old_value = unsafe { self.read_reg_idx(idx) };

        let mask = ((1 << len) - 1) << bit;
        let new_value = (old_value & !mask) | ((value << bit) & mask);

        unsafe { self.write_reg_idx(idx, new_value) };
    }

    /// # Safety
    /// The index must be a valid writable register index.
    unsafe fn write_reg_idx(&self, idx: u32, value: u32) {
        unsafe { self.reg_select().write(idx) };
        unsafe { self.reg_window().write(value) };
    }

    #[must_use]
    #[inline]
    fn read_reg(&self, reg: IoApicReg) -> u32 {
        unsafe { self.read_reg_idx(reg.index()) }
    }

    #[must_use]
    /// # Safety
    /// The index must be a valid register index.
    unsafe fn read_reg_idx(&self, idx: u32) -> u32 {
        unsafe { self.reg_select().write(idx) };
        unsafe { self.reg_window().read() }
    }

    #[must_use]
    #[inline]
    const fn reg_select(&self) -> Volatile<u32> {
        self.base.change_access(Access::WriteOnly)
    }

    #[must_use]
    #[inline]
    const fn reg_window(&self) -> Volatile<u32> {
        unsafe { self.base.byte_add(0x10) }
    }
}
