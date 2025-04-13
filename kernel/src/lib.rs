#![feature(abi_x86_interrupt, naked_functions)]
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_panics_doc,
    clippy::similar_names,
    clippy::missing_errors_doc,
    clippy::doc_markdown
)]

use hyperdrive::once::Once;

mod arch;
pub mod boot;
pub mod drivers;
pub mod locals;
pub mod log;
mod mem;
mod network;
pub mod process;
mod screen;
mod storage;
mod syscall;
pub mod time;

extern crate alloc;

static KERNEL_PANIC: Once<()> = Once::uninit();

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    arch::interrupts::int_disable();

    #[cfg(debug_assertions)]
    crate::error!("[PANIC]: Core {} - {}", locals!().core_id(), panic_info);
    #[cfg(not(debug_assertions))]
    crate::error!(
        "[PANIC]: Core {} - {}",
        locals!().core_id(),
        panic_info.message()
    );

    if process::scheduler::is_scheduling_init() {
        use crate::arch::apic::ipi;

        if process::scheduler::current_process().kind() == process::Kind::Kernel {
            // If a kernel (vital) process panics, crash the whole system.
            KERNEL_PANIC.call_once(|| {
                crate::error!("Kernel process panicked. Sending NMI to all cores.");
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

fn kernel_has_panicked() -> bool {
    KERNEL_PANIC.get().is_some()
}
