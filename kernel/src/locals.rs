use core::{ptr::NonNull, sync::atomic::AtomicUsize};

use alloc::boxed::Box;

use crate::arch::{apic::LocalApic, gdt::Gdt, interrupts::Interrupts};
use hyperdrive::{
    locks::mcs::{MUMcsLock, McsLock},
    once::Once,
};

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
    let apic_id = crate::arch::apic::apic_id();

    let mut core_locals = Box::new(CoreLocalsInfo {
        core_id,
        apic_id,
        ..CoreLocalsInfo::empty()
    });

    ALL_CORE_LOCALS[core_id].call_once(|| unsafe { NonNull::new_unchecked(core_locals.as_mut()) });

    crate::arch::locals::store_locals(Box::leak(core_locals));

    CORE_READY.fetch_add(1, core::sync::atomic::Ordering::Release);
}

#[must_use]
#[inline]
/// Returns the number of currently active cores
pub fn get_ready_core_count() -> usize {
    CORE_READY.load(core::sync::atomic::Ordering::Acquire)
}

#[must_use]
#[inline]
pub(crate) fn get_jumped_core_count() -> usize {
    CORE_JUMPED.load(core::sync::atomic::Ordering::Acquire)
}

#[inline]
/// Increment the count of cores that jumped to Rust code
pub(crate) fn core_jumped() {
    CORE_JUMPED.fetch_add(1, core::sync::atomic::Ordering::Acquire);
}

pub struct CoreLocalsInfo {
    core_id: usize,
    apic_id: u8,
    gdt: McsLock<Gdt>,
    interrupts: Interrupts,
    lapic: MUMcsLock<LocalApic>,
    scheduler: Once<crate::process::scheduler::Scheduler>,
}

impl CoreLocalsInfo {
    #[must_use]
    #[inline]
    pub const fn empty() -> Self {
        Self {
            core_id: 0,
            apic_id: 0,
            gdt: McsLock::new(Gdt::uninit()),
            interrupts: Interrupts::new(),
            lapic: MUMcsLock::uninit(),
            scheduler: Once::uninit(),
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
    pub const fn gdt(&self) -> &McsLock<Gdt> {
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

    #[must_use]
    #[inline]
    pub const fn scheduler(&self) -> &Once<crate::process::scheduler::Scheduler> {
        &self.scheduler
    }
}

#[must_use]
#[inline]
/// Returns a specific core local info
pub fn get_specific_core_locals(core_id: usize) -> Option<&'static CoreLocalsInfo> {
    ALL_CORE_LOCALS[core_id]
        .get()
        .copied()
        .map(|ptr| unsafe { ptr.as_ref() })
}

#[must_use]
#[inline]
/// Returns this core's local info.
pub fn get_core_locals() -> &'static CoreLocalsInfo {
    crate::arch::locals::load_locals()
}

/// A macro returning this core's local info.
#[macro_export]
macro_rules! locals {
    () => {
        $crate::locals::get_core_locals()
    };
}
