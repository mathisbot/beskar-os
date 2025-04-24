use core::marker::PhantomData;

use crate::arch::commons::VirtAddr;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
/// The interrupt stack frame pushed by the CPU on interrupt or exception entry.
pub struct InterruptStackFrame {
    instruction_pointer: VirtAddr,
    code_segment: u16,
    _reserved1: [u8; 6],
    cpu_flags: u64,
    stack_pointer: VirtAddr,
    /// Zero (long mode)
    stack_segment: u16,
    _reserved2: [u8; 6],
}

trait Sealed {}
#[allow(private_bounds)]
pub trait IdtFnPtr: Sealed {
    fn addr(&self) -> VirtAddr;
}

macro_rules! impl_idt_fn_ptr {
    ($($name:ident),+) => {
        $(
            impl Sealed for $name {}
            impl IdtFnPtr for $name {
                fn addr(&self) -> VirtAddr {
                    let addr = *self as *const () as u64;
                    unsafe { VirtAddr::new_unchecked(addr) }
                }
            }
        )+
    };
}

#[derive(Clone, Copy)]
#[repr(C)]
/// An entry in the Interrupt Descriptor Table (IDT).
pub struct IdtEntry<T: IdtFnPtr> {
    ptr_low: u16,
    options_cs: u16,
    options: u16,
    ptr_mid: u16,
    ptr_high: u32,
    _reserved: u32,
    _phantom: PhantomData<T>,
}

impl Default for IdtEntry<Handler> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T: IdtFnPtr> IdtEntry<T> {
    #[must_use]
    #[inline]
    pub const fn empty() -> Self {
        Self {
            ptr_low: 0,
            options_cs: 0,
            options: 0b1110_0000_0000, // 64-bit interrupt gate
            ptr_mid: 0,
            ptr_high: 0,
            _reserved: 0,
            _phantom: PhantomData,
        }
    }

    #[allow(clippy::needless_pass_by_value)] // Value is 8 bytes
    pub fn set_handler_fn(&mut self, handler: T, cs: u16) {
        let addr = handler.addr().as_u64();
        self.ptr_low = u16::try_from(addr & 0xFFFF).unwrap();
        self.ptr_mid = u16::try_from((addr >> 16) & 0xFFFF).unwrap();
        self.ptr_high = u32::try_from((addr >> 32) & 0xFFFF_FFFF).unwrap();

        self.options_cs = cs;
        self.options = 0b1110_0000_0000; // 64-bit interrupt gate
        self.options |= 1 << 15; // Present bit
    }

    /// Set the stack index for this IDT entry.
    ///
    /// # Panics
    ///
    /// The function panics if stack_index is outside of `0..=6`.
    ///
    /// # Safety
    ///
    /// The caller ensures the stack index is valid.
    pub unsafe fn set_stack_index(&mut self, stack_index: u8) {
        assert!(stack_index < 7, "Stack index must be less than 8");
        let real_index = stack_index + 1; // IST index starts at 1
        self.options = (self.options & 0xFFF8) | u16::from(real_index);
    }

    #[must_use]
    pub fn handler_vaddr(&self) -> VirtAddr {
        let addr = (u64::from(self.ptr_high) << 32)
            | (u64::from(self.ptr_mid) << 16)
            | u64::from(self.ptr_low);
        // Safety: pointers are canonical virtual addresses.
        unsafe { VirtAddr::new_unchecked(addr) }
    }
}

type Handler = extern "x86-interrupt" fn(InterruptStackFrame);
type HandlerErr = extern "x86-interrupt" fn(InterruptStackFrame, u64);

// TODO: When never type is stabilized, use Handler<T>.
type HandlerNever = extern "x86-interrupt" fn(InterruptStackFrame) -> !;
type HandlerErrNever = extern "x86-interrupt" fn(InterruptStackFrame, u64) -> !;

pub type PageFaultHandlerFunc = extern "x86-interrupt" fn(InterruptStackFrame, PageFaultErrorCode);

impl_idt_fn_ptr!(
    Handler,
    HandlerErr,
    HandlerNever,
    HandlerErrNever,
    PageFaultHandlerFunc
);

/// Interrupt Descriptor Table
///
/// The first 32 entries are reserved for CPU exceptions.
/// Entries 32 through 255 are used for user interrupts.
#[derive(Clone)]
#[repr(C)]
#[repr(align(16))]
pub struct InterruptDescriptorTable {
    /// A divide error (`#DE`) occurs when the denominator of a DIV/IDIV instruction is 0.
    /// A `#DE` also occurs if the result is too large to be represented in the destination.
    ///
    /// The saved instruction pointer points to the instruction that caused the `#DE`.
    ///
    /// The vector number of the `#DE` exception is 0.
    pub divide_error: IdtEntry<Handler>,

    /// A debug exception (`#DB`) can occur under many conditions.
    /// `#DB`  are enabled and disabled using the debug-control register, `DR7` and `RFLAGS.TF`.
    ///
    /// The saved instruction pointer depends on what triggered the `#DB`.
    /// If it is an instruction or an invalid debug register access,
    /// it points to the instruction that caused the `#DB`.
    /// Otherwise, it points to the instruction after.
    ///
    /// The vector number of the `#DB` exception is 1.
    pub debug: IdtEntry<Handler>,

    /// A non maskable interrupt exception (NMI) occurs as a result of system logic
    /// signaling a non-maskable interrupt to the processor.
    ///
    /// The processor recognizes an NMI at an instruction boundary.
    /// The saved instruction pointer points to the instruction immediately following the
    /// boundary where the NMI was recognized.
    ///
    /// The vector number of the NMI exception is 2.
    pub non_maskable_interrupt: IdtEntry<Handler>,

    /// A breakpoint (`#BP`) exception occurs when an `INT3` instruction is executed.
    ///
    /// The saved instruction pointer points to the byte after the `INT3` instruction.
    ///
    /// The vector number of the `#BP` exception is 3.
    pub breakpoint: IdtEntry<Handler>,

    /// An overflow exception (`#OF`) occurs as a result of executing an `INTO` instruction
    /// with the overflow bit in `RFLAGS` set.
    ///
    /// The saved instruction pointer points to the instruction following the `INTO`
    /// instruction that caused the `#OF`.
    ///
    /// The vector number of the `#OF` exception is 4.
    pub overflow: IdtEntry<Handler>,

    /// A bound-range exception (`#BR`) exception can occur as a result of executing
    /// the `BOUND` instruction.
    /// If the array index is not within the array boundary, the `#BR` occurs.
    ///
    /// The saved instruction pointer points to the `BOUND` instruction that caused the `#BR`.
    ///
    /// The vector number of the `#BR` exception is 5.
    pub bound_range_exceeded: IdtEntry<Handler>,

    /// An invalid opcode exception (`#UD`) occurs when an attempt is made to execute an
    /// invalid or undefined opcode. The validity of an opcode often depends on the
    /// processor operating mode.
    ///
    /// The saved instruction pointer points to the instruction that caused the `#UD`.
    ///
    /// The vector number of the `#UD` exception is 6.
    pub invalid_opcode: IdtEntry<Handler>,

    /// A device not available exception (`#NM`) occurs when an attempt is made to execute
    /// an x87 floating-point instruction or an SSE instruction while the x87 FPU or SSE
    /// unit is not available.
    ///
    /// The saved instruction pointer points to the instruction that caused the `#NM`.
    ///
    /// The vector number of the `#NM` exception is 7.
    pub device_not_available: IdtEntry<Handler>,

    /// A double fault (`#DF`) exception can occur when a second exception occurs during
    /// the handling of a prior critical (namely "contributory") exception.
    ///
    /// The returned error code is always zero. The saved instruction pointer is undefined
    /// as the program cannot be restarted.
    ///
    /// The vector number of the `#DF` exception is 8.
    pub double_fault: IdtEntry<HandlerErrNever>,

    /// This interrupt vector is reserved. It is for a discontinued exception originally used
    /// by processors that supported external x87-instruction coprocessors.
    _coprocessor_segment_overrun: IdtEntry<Handler>,

    /// An invalid TSS exception (`#TS`) occurs only as a result of a control transfer through
    /// a gate descriptor that results in an invalid stack-segment reference using an `SS`
    /// selector in the TSS.
    ///
    /// The returned error code is the `SS` segment selector. The saved instruction pointer
    /// points to the control-transfer instruction that caused the `#TS`.
    ///
    /// The vector number of the `#TS` exception is 10.
    pub invalid_tss: IdtEntry<HandlerErr>,

    /// An segment-not-present exception (`#NP`) occurs when an attempt is made to load a
    /// segment or gate with a clear present bit.
    ///
    /// The returned error code is the segment-selector index of the segment descriptor
    /// causing the `#NP` exception. The saved instruction pointer points to the instruction
    /// that loaded the segment selector resulting in the `#NP`.
    ///
    /// The vector number of the `#NP` exception is 11.
    pub segment_not_present: IdtEntry<HandlerErr>,

    /// An stack segment exception (`#SS`) can occur when an operation involving the stack
    /// (such as push, pop or call) is attempted with a cleared present bit in the stack segment
    /// or with a non-canonical address.
    ///
    /// The returned error code depends on the cause of the `#SS`. If the cause is a cleared
    /// present bit, the error code is the corresponding segment selector. Otherwise, the
    /// error code is zero. The saved instruction pointer points to the instruction that
    /// caused the `#SS`.
    ///
    /// The vector number of the `#NP` exception is 12.
    pub stack_segment_fault: IdtEntry<HandlerErr>,

    /// A general protection fault (`#GP`) can occur in various situations, including executing
    /// privileged instructions in user mode, writing a 1 into reserved register fields,
    /// attempting to execute unaligned SSE instructions, loading non-canonical addresses as GDT/IDT,
    /// writing read-only MSRs, ...
    ///
    /// The returned error code is a segment selector, if the cause of the `#GP` is
    /// segment-related, and zero otherwise. The saved instruction pointer points to
    /// the instruction that caused the `#GP`.
    ///
    /// The vector number of the `#GP` exception is 13.
    pub general_protection_fault: IdtEntry<HandlerErr>,

    /// A page fault (`#PF`) can occur during a memory access if the page isn't mapped/present,
    /// if CPU tries to fetch an instruction from a non-executable page, or if the access violates the
    /// paging-protection checks (user, write).
    ///
    /// The virtual address that caused the `#PF` is stored in the `CR2` register.
    /// The saved instruction pointer points to the instruction that caused the `#PF`.
    ///
    /// The vector number of the `#PF` exception is 14.
    pub page_fault: IdtEntry<PageFaultHandlerFunc>,

    /// Reserved exception vector 15
    _reserved1: IdtEntry<Handler>,

    /// 32-bit mode only: The x87 Floating-Point Exception-Pending exception (`#MF`)
    /// is used to handle unmasked x87 floating-point exceptions.
    ///
    /// The vector number of the `#MF` exception is 16.
    pub x87_floating_point: IdtEntry<Handler>,

    /// An alignment check exception (`#AC`) occurs when an unaligned-memory data reference
    /// is performed while alignment checking is enabled. An `#AC` can occur only when CPL=3.
    ///
    /// The returned error code is always zero. The saved instruction pointer points to the
    /// instruction that caused the `#AC`.
    ///
    /// The vector number of the `#AC` exception is 17.
    pub alignment_check: IdtEntry<HandlerErr>,

    /// The machine check exception (`#MC`) is model specific. Processor implementations
    /// are not required to support the `#MC` exception, and those implementations that do
    /// support `#MC` can vary in how the `#MC` exception mechanism works.
    ///
    /// There is no reliable way to restart the program.
    ///
    /// The vector number of the `#MC` exception is 18.
    pub machine_check: IdtEntry<HandlerNever>,

    /// The SIMD Floating-Point Exception (`#XF`) is used to handle unmasked SSE
    /// floating-point exceptions. The SSE floating-point exceptions reported by
    /// the `#XF` exception are (including mnemonics):
    ///
    /// - IE: Invalid-operation exception (also called #I).
    /// - DE: Denormalized-operand exception (also called #D).
    /// - ZE: Zero-divide exception (also called #Z).
    /// - OE: Overflow exception (also called #O).
    /// - UE: Underflow exception (also called #U).
    /// - PE: Precision exception (also called #P or inexact-result exception).
    ///
    /// The saved instruction pointer points to the instruction that caused the `#XF`.
    ///
    /// The vector number of the `#XF` exception is 19.
    pub simd_floating_point: IdtEntry<Handler>,

    /// Unused exception vector 20
    virtualization: IdtEntry<Handler>,

    /// A `#CP` exception is generated when shadow stacks are enabled and mismatch
    /// scenarios are detected (possible error code cases below).
    ///
    /// The error code is the `#CP` error code, for each of the following situations:
    /// - A RET (near) instruction encountered a return address mismatch.
    /// - A RET (far) instruction encountered a return address mismatch.
    /// - A RSTORSSP instruction encountered an invalid shadow stack restore token.
    /// - A SETSSBY instruction encountered an invalid supervisor shadow stack token.
    /// - A missing ENDBRANCH instruction if indirect branch tracking is enabled.
    ///
    /// The vector number of the `#CP` exception is 19.
    pub cp_protection_exception: IdtEntry<HandlerErr>,

    /// Vectors 20 through 27 are reserved.
    _reserved2: [IdtEntry<Handler>; 6],

    /// The Hypervisor Injection Exception (`#HV`) is injected by a hypervisor
    /// as a doorbell to inform an `SEV-SNP` enabled guest running with the
    /// `Restricted Injection` feature of events to be processed.
    ///
    /// The vector number of the ``#HV`` exception is 28.
    pub hv_injection_exception: IdtEntry<Handler>,

    /// The VMM Communication Exception (`#VC`) is always generated by hardware when an `SEV-ES`
    /// enabled guest is running and an `NAE` event occurs.
    ///
    /// The vector number of the ``#VC`` exception is 29.
    pub vmm_communication_exception: IdtEntry<HandlerErr>,

    /// The Security Exception (`#SX`) signals security-sensitive events that occur while
    /// executing the VMM, in the form of an exception so that the VMM may take appropriate
    /// action. (A VMM would typically intercept comparable sensitive events in the guest.)
    /// In the current implementation, the only use of the `#SX` is to redirect external INITs
    /// into an exception so that the VMM may — among other possibilities.
    ///
    /// The only error code currently defined is 1, and indicates redirection of INIT has occurred.
    ///
    /// The vector number of the ``#SX`` exception is 30.
    pub security_exception: IdtEntry<HandlerErr>,

    /// Vector 31
    _reserved3: IdtEntry<Handler>,

    /// User-defined interrupts.
    interrupts: [IdtEntry<Handler>; 256 - 32],
}

impl Default for InterruptDescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}

impl InterruptDescriptorTable {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            divide_error: IdtEntry::empty(),
            debug: IdtEntry::empty(),
            non_maskable_interrupt: IdtEntry::empty(),
            breakpoint: IdtEntry::empty(),
            overflow: IdtEntry::empty(),
            bound_range_exceeded: IdtEntry::empty(),
            invalid_opcode: IdtEntry::empty(),
            device_not_available: IdtEntry::empty(),
            double_fault: IdtEntry::empty(),
            _coprocessor_segment_overrun: IdtEntry::empty(),
            invalid_tss: IdtEntry::empty(),
            segment_not_present: IdtEntry::empty(),
            stack_segment_fault: IdtEntry::empty(),
            general_protection_fault: IdtEntry::empty(),
            page_fault: IdtEntry::empty(),
            _reserved1: IdtEntry::empty(),
            x87_floating_point: IdtEntry::empty(),
            alignment_check: IdtEntry::empty(),
            machine_check: IdtEntry::empty(),
            simd_floating_point: IdtEntry::empty(),
            virtualization: IdtEntry::empty(),
            cp_protection_exception: IdtEntry::empty(),
            _reserved2: [IdtEntry::empty(); 6],
            hv_injection_exception: IdtEntry::empty(),
            vmm_communication_exception: IdtEntry::empty(),
            security_exception: IdtEntry::empty(),
            _reserved3: IdtEntry::empty(),

            interrupts: [IdtEntry::<Handler>::empty(); 256 - 32],
        }
    }

    #[must_use]
    #[inline]
    pub fn irq(&mut self, index: u8) -> &mut IdtEntry<Handler> {
        let offset_idx = index.checked_sub(32).unwrap();
        &mut self.interrupts[usize::from(offset_idx)]
    }

    pub fn load(&'static self) {
        let descriptor = DescriptorTable::new(
            VirtAddr::from_ptr(self),
            u16::try_from(size_of::<Self>() - 1).unwrap(),
        );

        unsafe { super::instructions::load_idt(&descriptor) };
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
#[repr(transparent)]
/// Page Fault Error Code
pub struct PageFaultErrorCode(u64);

impl PageFaultErrorCode {
    pub const PROTECTION_VIOLATION: Self = Self(1);
    pub const WRITE: Self = Self(1 << 1);
    pub const USER_MODE: Self = Self(1 << 2);
    pub const MALFORMED_TABLE: Self = Self(1 << 3);
    pub const INSTRUCTION_FETCH: Self = Self(1 << 4);
    pub const PROTECTION_KEY: Self = Self(1 << 5);
    pub const SHADOW_STACK: Self = Self(1 << 6);
    pub const INTEL_SGX: Self = Self(1 << 15);
    pub const AMD_RMP: Self = Self(1 << 31);
}

impl core::fmt::Binary for PageFaultErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PageFaultErrorCode")
            .field("0", &format_args!("{:#b}", self.0))
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed(2))]
pub struct DescriptorTable {
    limit: u16,
    base: VirtAddr,
}

impl DescriptorTable {
    #[must_use]
    #[inline]
    pub const fn new(base: VirtAddr, limit: u16) -> Self {
        Self { limit, base }
    }

    #[must_use]
    #[inline]
    pub const fn base(&self) -> VirtAddr {
        self.base
    }

    #[must_use]
    #[inline]
    pub const fn limit(&self) -> u16 {
        self.limit
    }
}
