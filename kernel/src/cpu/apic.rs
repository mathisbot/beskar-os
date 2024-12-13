//! Advanced Programmable Interrupt Controller (APIC) driver.

use core::num::NonZeroU64;

use x86_64::{
    instructions::port::Port,
    structures::paging::{Mapper, PageTableFlags, PhysFrame, Size4KiB},
    PhysAddr,
};

use crate::{
    cpu::cpuid,
    locals,
    mem::{frame_alloc, page_alloc, page_table},
};

pub mod ap;

pub fn apic_id() -> u8 {
    let cpuid_res = cpuid::cpuid(1);
    u8::try_from((cpuid_res.ebx >> 24) & 0xFF).unwrap()
}

pub fn init() {
    let x2apic_supported = cpuid::check_feature(cpuid::CpuFeature::X2APIC);
    if !x2apic_supported {
        log::warn!("X2APIC not supported");
    }

    let lapic_paddr = crate::boot::acpi::ACPI.get().map_or_else(
        Apic::get_paddr_from_msr,
        crate::boot::acpi::Acpi::lapic_paddr,
    );

    ensure_pic_disabled();

    let apic = Apic::from_paddr(lapic_paddr);

    // TODO: Calibrate APIC timer when Timer is implemented

    locals!().apic().init(apic);
}

pub struct Apic {
    base: NonZeroU64,
    // TODO: Timer?
}

impl Apic {
    fn get_paddr_from_msr() -> PhysAddr {
        let msr = x86_64::registers::model_specific::Msr::new(0x1B);
        let base = NonZeroU64::new(unsafe { msr.read() }).unwrap();

        assert!((base.get() >> 11) & 1 == 1, "APIC not enabled");
        assert_eq!(
            (base.get() >> 8) & 1 == 1,
            locals!().core_id() == 0,
            "BSP incorrectly set"
        );

        let base_addr = (base.get()) & 0xF_FFFF_F000;

        PhysAddr::new(base_addr)
    }

    pub fn from_paddr(paddr: PhysAddr) -> Self {
        let phys_frame = PhysFrame::<Size4KiB>::from_start_address(paddr).unwrap();

        let apic_flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE
            | PageTableFlags::NO_CACHE;

        let page = page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.allocate_pages::<Size4KiB>(1).unwrap().start
        });

        frame_alloc::with_frame_allocator(|frame_allocator| {
            page_table::with_page_table(|page_table| {
                unsafe { page_table.map_to(page, phys_frame, apic_flags, &mut *frame_allocator) }
                    .unwrap()
                    .flush();
            });
        });

        // Register spurious interrupt handler
        let base_ptr: *mut u32 = page.start_address().as_mut_ptr();
        let apic_spurious = unsafe { &mut *base_ptr.add(0xF0 / size_of::<u32>()) };
        *apic_spurious &= !0xFF; // Clear spurious handler index
        *apic_spurious |= u32::from(super::interrupts::KernelInterrupts::Spurious as u8); // Set spurious handler index
        *apic_spurious |= 0x100; // Enable spurious interrupt

        Self {
            base: NonZeroU64::new(page.start_address().as_u64()).unwrap(),
        }
    }

    // TODO: Refactor sending IPIs
    /// Sends IPI to all APs, depending on the `sipi` parameter.
    ///
    /// If `sipi` is `None`, the APs will be sent an INIT.
    /// If `sipi` is `Some(payload)`, the APs will be sent a SIPI with `payload`.
    pub fn send_sipi(&self, sipi: Option<u8>) {
        let icr_low = (self.base.get() + 0x300) as *mut u32;

        while (unsafe { icr_low.read() >> 12 } & 1) == 1 {
            core::hint::spin_loop();
        }

        let low = {
            let mut low = 0;
            low |= 1 << 14; // Assert IPI should always be 1

            // If SIPI, set payload
            if let Some(payload) = sipi {
                low |= u32::from(payload);
            }

            // Set delivery mode
            low |= match sipi {
                Some(_) => 0b110, // SIPI
                None => 0b101,    // INIT
            } << 8;

            // Set destination
            low |= 0b11 << 18;

            low
        };

        unsafe { icr_low.write(low) };
    }
}

/// Ensures that PIC 8259 is disabled.
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
