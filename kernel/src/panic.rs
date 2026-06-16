use crate::arch::apic::ipi;
use crate::locals;
use crate::process::scheduler;
use core::fmt::Display;
use hyperdrive::once::Once;

static KERNEL_PANIC: Once<()> = Once::uninit();

pub(crate) fn panic_entry(msg: &dyn Display) -> ! {
    beskar_hal::instructions::int_disable();
    kernel_panic(&msg);
}

pub fn user_panic(msg: &dyn Display) -> ! {
    crate::error!("[PANIC] User process panicked: {}", msg);
    unsafe { scheduler::exit_current_thread() };
}

/// Panics the entire system.
pub fn kernel_panic(msg: &dyn Display) -> ! {
    beskar_hal::instructions::int_disable();
    KERNEL_PANIC.call_once(|| {
        let ipi_nmi = ipi::Ipi::new(ipi::DeliveryMode::Nmi, ipi::Destination::AllExcludingSelf);
        unsafe { locals!().lapic().force_lock() }.send_ipi(&ipi_nmi);
        // FIXME: While the system is unlikely to panic during logging,
        // NMI can be received at any time, including during logging
        // (resulting in a deadlock if the screen is locked).
        // TODO: BSOD
        crate::error!("Kernel panic! {}", msg);
    });

    // TODO: Attempt a gracious shutdown/reboot
    loop {
        crate::arch::halt();
    }
}

#[must_use]
#[inline]
/// Returns true if a core has panicked in a kernel thread.
pub fn kernel_has_panicked() -> bool {
    // We are not using `Once::is_initialized` here because we want
    // to catch the "still initializing" case as well (`get` is blocking).
    KERNEL_PANIC.get().is_some()
}
