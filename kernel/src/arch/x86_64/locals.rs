use crate::process::scheduler::thread::Tls;
use alloc::boxed::Box;
use beskar_core::arch::VirtAddr;
use beskar_hal::registers::{FS, GS};
use core::sync::atomic::{AtomicPtr, Ordering};
use hyperdrive::{
    locks::mcs::{MUMcsLock, McsLock},
    once::Once,
};

pub fn init(core_id: usize) -> &'static CoreLocalsInfo {
    // Obtain the APIC ID for this core
    let apic_id = super::apic::apic_id();

    // Create a new CoreLocalsInfo instance
    let locals = CoreLocalsInfo::new(core_id, apic_id);

    // Store the locals in the GS register
    store_locals(locals);

    locals
}

/// Per-core local storage.
pub struct CoreLocalsInfo {
    self_ptr: AtomicPtr<Self>,

    core_id: usize,
    scheduler: Once<crate::process::scheduler::Scheduler>,

    // Arch specific fields
    apic_id: u8,
    gdt: McsLock<super::gdt::Gdt>,
    interrupts: super::interrupts::Interrupts,
    lapic: MUMcsLock<super::apic::LocalApic>,
}

impl CoreLocalsInfo {
    #[must_use]
    #[inline]
    const fn new_impl(core_id: usize, apic_id: u8) -> Self {
        Self {
            self_ptr: AtomicPtr::new(core::ptr::null_mut()),
            core_id,
            scheduler: Once::uninit(),
            apic_id,
            gdt: McsLock::new(super::gdt::Gdt::uninit()),
            interrupts: super::interrupts::Interrupts::new(),
            lapic: MUMcsLock::uninit(),
        }
    }

    #[must_use]
    pub fn new(core_id: usize, apic_id: u8) -> &'static Self {
        let locals = Box::leak(Box::new(Self::new_impl(core_id, apic_id)));
        locals.set_self_ptr();
        locals
    }

    #[inline]
    fn set_self_ptr(&self) {
        self.self_ptr
            .store(core::ptr::from_ref(self).cast_mut(), Ordering::Relaxed);
    }

    #[must_use]
    #[inline]
    pub const fn core_id(&self) -> usize {
        self.core_id
    }

    #[must_use]
    #[inline]
    pub const fn scheduler(&self) -> &Once<crate::process::scheduler::Scheduler> {
        &self.scheduler
    }

    #[must_use]
    #[inline]
    pub const fn apic_id(&self) -> u8 {
        self.apic_id
    }

    #[must_use]
    #[inline]
    pub const fn gdt(&self) -> &McsLock<super::gdt::Gdt> {
        &self.gdt
    }

    #[must_use]
    #[inline]
    pub const fn interrupts(&self) -> &super::interrupts::Interrupts {
        &self.interrupts
    }

    #[must_use]
    #[inline]
    pub const fn lapic(&self) -> &MUMcsLock<super::apic::LocalApic> {
        &self.lapic
    }
}

#[cold]
/// Stores a CoreLocalsInfo instance for the current core by setting the GS register.
///
/// This should be called exactly once per core during initialization.
fn store_locals(locals: &'static CoreLocalsInfo) {
    unsafe {
        GS::write_base(VirtAddr::from_ptr(core::ptr::from_ref(locals)));
    }
}

/// Retrieves the CoreLocalsInfo for the current core via the GS register.
#[must_use]
#[inline]
pub fn load_locals() -> &'static CoreLocalsInfo {
    // Safety:
    // The GS register is set to point to CoreLocalsInfo (via store_locals).
    // The self_ptr field is set to point to the CoreLocalsInfo itself during init.
    unsafe { &*GS::read_ptr(core::mem::offset_of!(CoreLocalsInfo, self_ptr)) }
}

#[inline]
/// Store the thread's local info.
pub fn store_thread_locals(tls: Tls) {
    unsafe { FS::write_base(tls.addr()) };
}
