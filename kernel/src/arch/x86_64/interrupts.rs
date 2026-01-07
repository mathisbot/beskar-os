use super::gdt::{DOUBLE_FAULT_IST, PAGE_FAULT_IST};
use crate::locals;
use beskar_core::arch::VirtAddr;
use beskar_hal::{
    instructions::int_enable,
    registers::{CS, Cr0, Cr2},
    structures::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
    userspace::Ring,
};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU8, Ordering},
};

pub fn init() {
    let interrupts = locals!().interrupts();

    let idt = unsafe { &mut *interrupts.idt.get() };

    // Exceptions

    let cs = CS::read();

    idt.divide_error.set_handler_fn(divide_error_handler, cs);
    idt.debug.set_handler_fn(debug_handler, cs);
    idt.non_maskable_interrupt
        .set_handler_fn(non_maskable_interrupt_handler, cs);
    unsafe {
        idt.breakpoint
            .set_handler_fn_unchecked(VirtAddr::from_ptr(breakpoint_handler as *const ()), cs);
    }
    idt.breakpoint.set_dpl(Ring::User);
    idt.overflow.set_handler_fn(overflow_handler, cs);
    idt.bound_range_exceeded
        .set_handler_fn(bound_range_exceeded_handler, cs);
    idt.invalid_opcode
        .set_handler_fn(invalid_opcode_handler, cs);
    idt.device_not_available
        .set_handler_fn(device_not_available_handler, cs);
    idt.invalid_tss.set_handler_fn(invalid_tss_handler, cs);
    idt.segment_not_present
        .set_handler_fn(segment_not_present_handler, cs);
    idt.stack_segment_fault
        .set_handler_fn(stack_segment_fault_handler, cs);
    idt.general_protection_fault
        .set_handler_fn(general_protection_fault_handler, cs);
    idt.x87_floating_point
        .set_handler_fn(x87_floating_point_handler, cs);
    idt.alignment_check
        .set_handler_fn(alignment_check_handler, cs);
    idt.machine_check.set_handler_fn(machine_check_handler, cs);
    idt.simd_floating_point
        .set_handler_fn(simd_floating_point_handler, cs);
    idt.cp_protection_exception
        .set_handler_fn(cp_protection_handler, cs);
    idt.hv_injection_exception
        .set_handler_fn(hv_injection_handler, cs);
    idt.vmm_communication_exception
        .set_handler_fn(vmm_communication_handler, cs);
    idt.security_exception
        .set_handler_fn(security_exception_handler, cs);

    idt.double_fault.set_handler_fn(double_fault_handler, cs);
    unsafe {
        idt.double_fault.set_stack_index(DOUBLE_FAULT_IST);
    }
    idt.page_fault.set_handler_fn(page_fault_handler, cs);
    unsafe {
        idt.page_fault.set_stack_index(PAGE_FAULT_IST);
    }

    idt.irq(0xFF)
        .unwrap()
        .set_handler_fn(spurious_interrupt_handler, cs);

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
    video::error!(
        "EXCEPTION: PAGE FAULT {:b} in Thread {}",
        error_code,
        crate::process::scheduler::current_thread_id().as_u64()
    );
    video::error!("Accessed Address: {:#x}", Cr2::read().as_u64());
    // video::error!("{:#?}", stack_frame);
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
        extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame, err_code: u64) {
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
        extern "x86-interrupt" fn $name(_stack_frame: InterruptStackFrame) {
            video::info!(
                "{} INTERRUPT on core {} - t{}",
                stringify!($name),
                locals!().core_id(),
                $crate::process::scheduler::current_thread_id().as_u64()
            );
        }
    };
}

panic_isr!(divide_error_handler);
info_isr!(debug_handler);
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
panic_isr_with_errcode!(cp_protection_handler);
panic_isr!(hv_injection_handler);
panic_isr_with_errcode!(vmm_communication_handler);
panic_isr_with_errcode!(security_exception_handler);

#[unsafe(naked)]
unsafe extern "C" fn breakpoint_handler() {
    core::arch::naked_asm!(
        // Save registers
        "push rax",
        "push rcx",
        "push rdx",
        "push rbx",
        "push rbp",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // rdx = &ThreadRegisters
        "mov rsi, rsp",
        // rsi = &InterruptStackFrame
        "lea rdi, [rsp + {size}]",

        // Align stack (rsp % 16 == 8 before call)
        "sub rsp, 8",
        "call {f}",
        "add rsp, 8",

        // Restore registers
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rbp",
        "pop rbx",
        "pop rdx",
        "pop rcx",
        "pop rax",

        "iretq",

        size = const size_of::<ThreadRegisters>(),
        f = sym breakpoint_handler_impl,
    );
}

extern "C" fn breakpoint_handler_impl(
    stack_frame: &InterruptStackFrame,
    registers: &ThreadRegisters,
) {
    video::debug!(
        "Breakpoint reached in Thread {} ({:?})\n{:#?}",
        crate::process::scheduler::current_thread_id().as_u64(),
        stack_frame.instruction_pointer().as_ptr::<()>(),
        registers,
    );
}

#[repr(C)]
/// Registers that are relevant for the thread context.
pub struct ThreadRegisters {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rbx: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
}

impl core::fmt::Debug for ThreadRegisters {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ThreadRegisters")
            .field("rax", &format_args!("{:#018x}", self.rax))
            .field("rcx", &format_args!("{:#018x}", self.rcx))
            .field("rdx", &format_args!("{:#018x}", self.rdx))
            .field("rbx", &format_args!("{:#018x}", self.rbx))
            .field("rbp", &format_args!("{:#018x}", self.rbp))
            .field("rsi", &format_args!("{:#018x}", self.rsi))
            .field("rdi", &format_args!("{:#018x}", self.rdi))
            .field("r8 ", &format_args!("{:#018x}", self.r8))
            .field("r9 ", &format_args!("{:#018x}", self.r9))
            .field("r10", &format_args!("{:#018x}", self.r10))
            .field("r11", &format_args!("{:#018x}", self.r11))
            .field("r12", &format_args!("{:#018x}", self.r12))
            .field("r13", &format_args!("{:#018x}", self.r13))
            .field("r14", &format_args!("{:#018x}", self.r14))
            .field("r15", &format_args!("{:#018x}", self.r15))
            .finish()
    }
}

#[expect(
    unreachable_code,
    reason = "FPU/SIMD state saving/restoring is not implemented yet"
)]
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

info_isr!(spurious_interrupt_handler);

#[inline]
/// Allocates a new IRQ handler in the IDT and return its index.
///
/// A CPU index may be passed to bind the IRQ to a specific CPU core.
pub fn new_irq(
    handler: extern "x86-interrupt" fn(InterruptStackFrame),
    core: Option<usize>,
) -> (u8, usize) {
    /// IDT index counter.
    ///
    /// It skips the first 32 entries, which are reserved for exceptions.
    // TODO: Per-core IRQ counters
    static IDX: AtomicU8 = AtomicU8::new(32);

    let core_id = core.unwrap_or_else(|| locals!().core_id());
    let core_locals = crate::locals::get_specific_core_locals(core_id).unwrap();

    let idx = IDX.fetch_add(1, Ordering::Relaxed);

    let idt = unsafe { &mut *core_locals.interrupts().idt.get() };
    let idt_entry = idt.irq(idx).expect("IRQ counter has overflown");

    assert_eq!(
        idt_entry.handler_vaddr(),
        VirtAddr::ZERO,
        "IRQ {idx} is already used",
    );
    idt_entry.set_handler_fn(handler, CS::read());

    (idx, core_id)
}

// Safety: access to the IDT is synchronized by an atomic index counter
unsafe impl Sync for Interrupts {}
