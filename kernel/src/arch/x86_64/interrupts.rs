use core::cell::UnsafeCell;

use beskar_core::arch::x86_64::registers::{Cr0, Cr2};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use super::gdt::{DOUBLE_FAULT_IST, PAGE_FAULT_IST};
use crate::locals;

pub fn init() {
    let interrupts = locals!().interrupts();

    let idt = unsafe { &mut *interrupts.idt.get() };

    // Exceptions

    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.debug.set_handler_fn(debug_handler);
    idt.non_maskable_interrupt
        .set_handler_fn(non_maskable_interrupt_handler);
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.overflow.set_handler_fn(overflow_handler);
    idt.bound_range_exceeded
        .set_handler_fn(bound_range_exceeded_handler);
    idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
    idt.device_not_available
        .set_handler_fn(device_not_available_handler);
    idt.invalid_tss.set_handler_fn(invalid_tss_handler);
    idt.segment_not_present
        .set_handler_fn(segment_not_present_handler);
    idt.stack_segment_fault
        .set_handler_fn(stack_segment_fault_handler);
    idt.general_protection_fault
        .set_handler_fn(general_protection_fault_handler);
    idt.x87_floating_point
        .set_handler_fn(x87_floating_point_handler);
    idt.alignment_check.set_handler_fn(alignment_check_handler);
    idt.machine_check.set_handler_fn(machine_check_handler);
    idt.simd_floating_point
        .set_handler_fn(simd_floating_point_handler);
    idt.virtualization.set_handler_fn(virtualization_handler);
    idt.cp_protection_exception
        .set_handler_fn(cp_protection_handler);
    idt.hv_injection_exception
        .set_handler_fn(hv_injection_handler);
    idt.vmm_communication_exception
        .set_handler_fn(vmm_communication_handler);
    idt.security_exception
        .set_handler_fn(security_exception_handler);

    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(DOUBLE_FAULT_IST)
    };
    unsafe {
        idt.page_fault
            .set_handler_fn(page_fault_handler)
            .set_stack_index(PAGE_FAULT_IST)
    };

    // IRQs

    idt[Irq::Timer as u8].set_handler_fn(timer_interrupt_handler);
    idt[Irq::Spurious as u8].set_handler_fn(spurious_interrupt_handler);

    // TODO: Allocate these at runtime
    // This is hard because they need to be set for all cores
    idt[Irq::Xhci as u8].set_handler_fn(xhci_interrupt_handler);
    idt[Irq::Nic as u8].set_handler_fn(nic_interrupt_handler);
    idt[Irq::Nvme as u8].set_handler_fn(nvme_interrupt_handler);
    idt[Irq::LocalNmi as u8].set_handler_fn(local_nmi_handler);
    idt[Irq::IoIso as u8].set_handler_fn(io_iso_handler);

    idt.load();

    crate::arch::interrupts::int_enable();
}

#[derive(Debug)]
pub struct Interrupts {
    idt: UnsafeCell<InterruptDescriptorTable>,
}

impl Default for Interrupts {
    fn default() -> Self {
        Self::new()
    }
}

impl Interrupts {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            idt: UnsafeCell::new(InterruptDescriptorTable::new()),
        }
    }
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!(
        "EXCEPTION: DOUBLE FAULT {:#x}\n{:#?}",
        error_code, stack_frame
    );
}

extern "x86-interrupt" fn page_fault_handler(
    _stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    crate::error!(
        "EXCEPTION: PAGE FAULT {:?} in Thread {}",
        error_code,
        crate::process::scheduler::current_thread_id().as_u64()
    );
    crate::error!("Accessed Address: {:#x}", Cr2::read().as_u64());
    // crate::error!("{:#?}", stack_frame);
    panic!();
}

macro_rules! panic_isr {
    ($name:ident) => {
        extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame) {
            panic!(
                "EXCEPTION: {} INTERRUPT on core {}\n{:#?}",
                stringify!($name),
                locals!().core_id(),
                stack_frame
            );
        }
    };
}

macro_rules! panic_isr_with_errcode {
    ($name:ident) => {
        extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame, err_code: u64) -> () {
            panic!(
                "EXCEPTION: {} INTERRUPT {:#x} on core {}\n{:#?}",
                stringify!($name),
                err_code,
                locals!().core_id(),
                stack_frame
            );
        }
    };
}

macro_rules! info_isr {
    ($name:ident) => {
        extern "x86-interrupt" fn $name(_stack_frame: InterruptStackFrame) -> () {
            crate::info!(
                "{} INTERRUPT on core {} - t{}",
                stringify!($name),
                locals!().core_id(),
                $crate::process::scheduler::current_thread_id().as_u64()
            );
        }
    };
}

macro_rules! info_isr_eoi {
    ($name:ident) => {
        extern "x86-interrupt" fn $name(_stack_frame: InterruptStackFrame) -> () {
            crate::info!(
                "{} INTERRUPT on core {}",
                stringify!($name),
                locals!().core_id()
            );
            unsafe { locals!().lapic().force_lock() }.send_eoi();
        }
    };
}

panic_isr!(divide_error_handler);
info_isr!(debug_handler);
panic_isr!(breakpoint_handler);
panic_isr!(overflow_handler);
panic_isr!(bound_range_exceeded_handler);
panic_isr!(invalid_opcode_handler);
panic_isr_with_errcode!(invalid_tss_handler);
panic_isr_with_errcode!(segment_not_present_handler);
panic_isr_with_errcode!(stack_segment_fault_handler);
panic_isr_with_errcode!(general_protection_fault_handler);
panic_isr!(x87_floating_point_handler);
panic_isr_with_errcode!(alignment_check_handler);
panic_isr!(simd_floating_point_handler);
panic_isr!(virtualization_handler);
panic_isr_with_errcode!(cp_protection_handler);
panic_isr!(hv_injection_handler);
panic_isr_with_errcode!(vmm_communication_handler);
panic_isr_with_errcode!(security_exception_handler);

extern "x86-interrupt" fn device_not_available_handler(_stack_frame: InterruptStackFrame) {
    let cr0 = Cr0::read();
    if cr0 & Cr0::TASK_SWITCHED != 0 {
        panic!("EXCEPTION: DEVICE NOT AVAILABLE");
    } else {
        // TODO: Save FPU/SIMD state
        // Choose between FXSAVE/FXRSTOR and XSAVE/XRSTOR
        // Maybe set MP flag in CR0 and keep the Thread ID of the last FPU user?
        todo!("Save FPU/SIMD state");
        todo!("Restore FPU/SIMD state");
        unsafe { Cr0::write(cr0 & !Cr0::TASK_SWITCHED) };
    }
}

extern "x86-interrupt" fn non_maskable_interrupt_handler(_stack_frame: InterruptStackFrame) {
    if crate::kernel_has_panicked() {
        panic!("Another Core has panicked in a kernel thread");
    } else {
        panic!("EXCEPTION: NON MASKABLE INTERRUPT");
    }
}

extern "x86-interrupt" fn machine_check_handler(_stack_frame: InterruptStackFrame) -> ! {
    panic!("EXCEPTION: MACHINE CHECK");
}

info_isr_eoi!(spurious_interrupt_handler);

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let rescheduling_result = crate::process::scheduler::reschedule();

    // Safety:
    // `send_eoi` is safe to use on locked LAPICs (see its documentation).
    // Also, the LAPIC is initialized if the interrupt has been received ;).
    unsafe { locals!().lapic().force_lock() }.send_eoi();

    if let Some(context_switch) = rescheduling_result {
        // Safety:
        // If rescheduling happened, interrupts were disabled.
        unsafe { context_switch.perform() };
    }
}

extern "x86-interrupt" fn xhci_interrupt_handler(_stack_frame: InterruptStackFrame) {
    crate::info!("xHCI INTERRUPT on core {}", locals!().core_id());
    crate::drivers::usb::host::handle_usb_interrupt();
    unsafe { locals!().lapic().force_lock() }.send_eoi();
}

extern "x86-interrupt" fn nic_interrupt_handler(_stack_frame: InterruptStackFrame) {
    crate::info!("NIC INTERRUPT on core {}", locals!().core_id());
    unsafe { locals!().lapic().force_lock() }.send_eoi();
}

extern "x86-interrupt" fn nvme_interrupt_handler(_stack_frame: InterruptStackFrame) {
    crate::info!("NVMe INTERRUPT on core {}", locals!().core_id());
    unsafe { locals!().lapic().force_lock() }.send_eoi();
}

extern "x86-interrupt" fn local_nmi_handler(_stack_frame: InterruptStackFrame) {
    crate::info!("Local NMI on core {}", locals!().core_id());
    unsafe { locals!().lapic().force_lock() }.send_eoi();
}

extern "x86-interrupt" fn io_iso_handler(_stack_frame: InterruptStackFrame) {
    crate::info!("IO ISO on core {}", locals!().core_id());
    unsafe { locals!().lapic().force_lock() }.send_eoi();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
/// Represents a programmable interrupt index
pub enum Irq {
    // As the 32 first interrupts are reserved for exceptions,
    // all numbers defined here must be greater than or equal to 32.
    Timer = 32,
    Spurious = 33,
    Xhci = 34,
    Nic = 35,
    Nvme = 36,
    LocalNmi = 37,
    IoIso = 38,
}

#[inline]
pub fn int_disable() {
    unsafe {
        core::arch::asm!("cli", options(nomem, preserves_flags, nostack));
    }
}

#[inline]
pub fn int_enable() {
    unsafe {
        core::arch::asm!("sti", options(nomem, preserves_flags, nostack));
    }
}
