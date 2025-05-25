#![feature(abi_x86_interrupt)]
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_panics_doc,
    clippy::similar_names,
    clippy::missing_errors_doc,
    clippy::doc_markdown
)]
extern crate alloc;
use hyperdrive::once::Once;

mod arch;
pub mod boot;
pub mod drivers;
pub mod locals;
mod mem;
pub mod process;
pub mod storage;
mod syscall;
mod time;

static KERNEL_PANIC: Once<()> = Once::uninit();

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    arch::interrupts::int_disable();

    #[cfg(debug_assertions)]
    video::error!("[PANIC]: Core {} - {}", locals!().core_id(), panic_info);
    #[cfg(not(debug_assertions))]
    video::error!(
        "[PANIC]: Core {} - {}",
        locals!().core_id(),
        panic_info.message()
    );

    // If more than one core is present, then both processes and APICs are initialized.
    if crate::locals::core_count() > 1 {
        use crate::arch::apic::ipi;

        if process::scheduler::current_process().kind() == beskar_hal::process::Kind::Kernel {
            // If a kernel (vital) process panics, crash the whole system.
            KERNEL_PANIC.call_once(|| {
                video::error!("Kernel process panicked. Sending NMI to all cores.");
                let ipi_nmi =
                    ipi::Ipi::new(ipi::DeliveryMode::Nmi, ipi::Destination::AllExcludingSelf);
                // FIXME: While the system is unlikely to panic during logging,
                // NMI can be received at any time, including during logging
                // (resulting in a deadlock if the screen is locked).
                unsafe { locals!().lapic().force_lock() }.send_ipi(&ipi_nmi);
                // TODO: BSOD
            });
        } else if !kernel_has_panicked() {
            // Otherwise, it should be safe to kill the process and proceed.
            unsafe { process::scheduler::exit_current_thread() };
        }
    }

    loop {
        crate::arch::halt();
    }
}

#[must_use]
#[inline]
/// Returns true if a core has panicked in a kernel thread.
fn kernel_has_panicked() -> bool {
    // We are not using `Once::is_initialized` here because we want
    // to catch the "still initializing" case as well (`get` is blocking).
    KERNEL_PANIC.get().is_some()
}
