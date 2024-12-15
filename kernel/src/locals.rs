use core::{ptr::NonNull, sync::atomic::AtomicU8};

use x86_64::registers::segmentation::Segment64;

use alloc::boxed::Box;

use crate::{
    cpu::{apic::LocalApic, gdt::Gdt, interrupts::Interrupts},
    utils::locks::MUMcsLock,
};

/// Count APs that got around of their trampoline code
///
/// It is initialized at 1 because the BSP is already in the Rust code ;)
static CORE_JUMPED: AtomicU8 = AtomicU8::new(1);
/// Count APs that are ready, right before entering `enter_kmain`
static CORE_READY: AtomicU8 = AtomicU8::new(0);
/// Distributes core IDs
static CORE_ID: AtomicU8 = AtomicU8::new(0);

// FIXME: Find a way to support an arbitrary number of cores (using a `Vec` makes it harder
// to correctly initialize without data races)
/// This array holds the core locals for each core, so that it is accessible from any core.
static mut ALL_CORE_LOCALS: [Option<NonNull<CoreLocalsInfo>>; 255] = [None; 255];

pub fn init() {
    let core_id = CORE_ID.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
    let apic_id = crate::cpu::apic::apic_id();

    // Size is too random to manually map it
    let mut core_locals = Box::new(CoreLocalsInfo {
        core_id,
        apic_id,
        // GDT will be initialized later
        ..CoreLocalsInfo::empty()
    });

    // Safety:
    // Each core only accesses its own entry in the array on startup.
    // The array is then never modified.
    unsafe {
        ALL_CORE_LOCALS[core_id as usize] = Some(NonNull::new(core_locals.as_mut()).unwrap());
    }

    unsafe {
        x86_64::registers::control::Cr4::update(|cr4| {
            cr4.insert(x86_64::registers::control::Cr4Flags::FSGSBASE);
        });
    }

    unsafe {
        x86_64::registers::segmentation::GS::write_base(x86_64::VirtAddr::new(
            core::ptr::from_ref(core_locals.as_ref()) as u64,
        ));
    }

    // Shouldn't be dropped
    core::mem::forget(core_locals);

    let _ = CORE_READY.fetch_add(1, core::sync::atomic::Ordering::Release);
}

pub fn get_ready_core_count() -> u8 {
    CORE_READY.load(core::sync::atomic::Ordering::SeqCst)
}

pub fn get_jumped_core_count() -> u8 {
    CORE_JUMPED.load(core::sync::atomic::Ordering::SeqCst)
}

/// Increment the count of cores that jumped to Rust code
pub fn core_jumped() {
    let _ = CORE_JUMPED.fetch_add(1, core::sync::atomic::Ordering::Release);
}

pub struct CoreLocalsInfo {
    core_id: u8,
    apic_id: u8,
    gdt: Gdt,
    interrupts: Interrupts,
    lapic: MUMcsLock<LocalApic>,
}

impl CoreLocalsInfo {
    #[must_use]
    #[inline]
    pub const fn empty() -> Self {
        Self {
            core_id: 0,
            apic_id: 0,
            gdt: Gdt::uninit(),
            interrupts: Interrupts::new(),
            lapic: MUMcsLock::uninit(),
        }
    }

    #[must_use]
    #[inline]
    pub const fn core_id(&self) -> u8 {
        self.core_id
    }

    #[must_use]
    #[inline]
    pub const fn apic_id(&self) -> u8 {
        self.apic_id
    }

    #[must_use]
    #[inline]
    pub const fn gdt(&self) -> &Gdt {
        &self.gdt
    }

    #[must_use]
    #[inline]
    pub const fn interrupts(&self) -> &Interrupts {
        &self.interrupts
    }

    #[must_use]
    #[inline]
    pub const fn lapic(&self) -> &MUMcsLock<LocalApic> {
        &self.lapic
    }
}

#[must_use]
#[inline]
/// Returns this core's local info.
pub fn get_core_locals() -> &'static CoreLocalsInfo {
    // Safety:
    // The GS register is set to point to the core's local info.
    unsafe { &*x86_64::registers::segmentation::GS::read_base().as_ptr() }
}

/// A macro returning this core's local info.
#[macro_export]
macro_rules! locals {
    () => {
        $crate::locals::get_core_locals()
    };
}
