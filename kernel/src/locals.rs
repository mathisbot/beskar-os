use crate::arch::{apic::LocalApic, gdt::Gdt, interrupts::Interrupts};
use alloc::boxed::Box;
use core::sync::atomic::{AtomicUsize, Ordering};
use hyperdrive::{
    locks::mcs::{MUMcsLock, McsLock},
    once::Once,
};

/// Distributes core IDs
static CORE_ID: AtomicUsize = AtomicUsize::new(0);

/// This array holds the core locals for each core, so that it is accessible from any core.
static ALL_CORE_LOCALS: [Once<&'static CoreLocalsInfo>; 256] = [const { Once::uninit() }; 256];

pub fn init() {
    let core_id = CORE_ID.fetch_add(1, Ordering::Relaxed);
    let apic_id = crate::arch::apic::apic_id();

    let core_locals = Box::new(CoreLocalsInfo {
        core_id,
        apic_id,
        ..CoreLocalsInfo::empty()
    });
    let core_locals = Box::leak(core_locals);

    ALL_CORE_LOCALS[core_id].call_once(|| core_locals);
    crate::arch::locals::store_locals(core_locals);
}

#[must_use]
#[inline]
/// Returns the number of currently active cores
pub fn core_count() -> usize {
    CORE_ID.load(core::sync::atomic::Ordering::Acquire)
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
    ALL_CORE_LOCALS[core_id].get().copied()
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
