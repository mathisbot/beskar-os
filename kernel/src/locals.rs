use core::{ptr::NonNull, sync::atomic::AtomicUsize};

use x86_64::{
    VirtAddr,
    registers::{
        control::{Cr4, Cr4Flags},
        segmentation::{GS, Segment64},
    },
};

use alloc::boxed::Box;

use crate::cpu::{apic::LocalApic, gdt::Gdt, interrupts::Interrupts};
use hyperdrive::{locks::mcs::MUMcsLock, once::Once};

/// Count APs that got around of their trampoline code
///
/// It is initialized at 1 because the BSP is already in the Rust code ;)
static CORE_JUMPED: AtomicUsize = AtomicUsize::new(1);
/// Count APs that are ready, right before entering `enter_kmain`
static CORE_READY: AtomicUsize = AtomicUsize::new(0);
/// Distributes core IDs
static CORE_ID: AtomicUsize = AtomicUsize::new(0);

// FIXME: Find a way to support an arbitrary number of cores (using a `Vec` makes it harder
// to correctly initialize without data races)
/// This array holds the core locals for each core, so that it is accessible from any core.
static ALL_CORE_LOCALS: [Once<NonNull<CoreLocalsInfo>>; 256] = [const { Once::uninit() }; 256];

pub fn init() {
    let core_id = CORE_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let apic_id = crate::cpu::apic::apic_id();

    let mut core_locals = Box::new(CoreLocalsInfo {
        core_id,
        apic_id,
        ..CoreLocalsInfo::empty()
    });

    ALL_CORE_LOCALS[core_id].call_once(|| unsafe { NonNull::new_unchecked(core_locals.as_mut()) });

    unsafe {
        Cr4::update(|cr4| cr4.insert(Cr4Flags::FSGSBASE));
    }

    unsafe {
        GS::write_base(VirtAddr::new(
            core::ptr::from_ref(core_locals.as_ref()) as u64
        ));
    }

    // Shouldn't be dropped
    core::mem::forget(core_locals);

    CORE_READY.fetch_add(1, core::sync::atomic::Ordering::Release);
}

/// Returns the number of currently active cores
pub fn get_ready_core_count() -> usize {
    CORE_READY.load(core::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn get_jumped_core_count() -> usize {
    CORE_JUMPED.load(core::sync::atomic::Ordering::Relaxed)
}

/// Increment the count of cores that jumped to Rust code
pub(crate) fn core_jumped() {
    CORE_JUMPED.fetch_add(1, core::sync::atomic::Ordering::Release);
}

pub struct CoreLocalsInfo {
    core_id: usize,
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
    pub const fn core_id(&self) -> usize {
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
