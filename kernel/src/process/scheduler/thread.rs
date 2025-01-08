use core::{
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
};

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use hyperdrive::queues::mpsc::{Link, Queueable};
use x86_64::registers::rflags::RFlags;

use super::{super::Process, priority::Priority};

/// The minimum amount of stack space that must be left unused on thread creation.
const MINIMUM_LEFTOVER_STACK: usize = 0x100; // 256 bytes

pub struct Thread {
    /// The unique identifier of the thread.
    id: ThreadId,
    /// The process that this thread belongs to.
    root_proc: Arc<Process>,
    /// The priority of the thread.
    priority: Priority,
    /// Used to keep ownership of the stack when needed.
    stack: Option<Vec<u8>>,
    /// Keeps track of where the stack is.
    ///
    /// The usize is the last stack pointer.
    /// The reason it is a pinned `Box` is so that we can easily get a reference to it
    /// and update it when switching contexts.
    pub(super) last_stack_ptr: Pin<Box<usize>>,

    /// Link to the next thread in the queue.
    pub(super) link: Link<Self>,
}

impl Unpin for Thread {}

impl Queueable for Thread {
    type Handle = Pin<Box<Self>>;

    unsafe fn capture(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        unsafe { Pin::new(Box::from_raw(ptr.as_ptr())) }
    }

    fn release(r: Self::Handle) -> core::ptr::NonNull<Self> {
        let ptr = Box::into_raw(Pin::into_inner(r));
        unsafe { core::ptr::NonNull::new_unchecked(ptr) }
    }

    unsafe fn get_link(ptr: core::ptr::NonNull<Self>) -> core::ptr::NonNull<Link<Self>> {
        let ptr = unsafe { &raw mut (*ptr.as_ptr()).link };
        unsafe { core::ptr::NonNull::new_unchecked(ptr) }
    }
}

impl Thread {
    #[must_use]
    #[inline]
    pub(crate) fn new_kernel(kernel_process: Arc<Process>) -> Self {
        Self {
            id: ThreadId::new(),
            root_proc: kernel_process,
            priority: Priority::High,
            stack: None,
            // Will be overwritten before being used.
            last_stack_ptr: Box::pin(0),
            link: Link::default(),
        }
    }

    #[must_use]
    #[inline]
    pub fn new(
        root_proc: Arc<Process>,
        priority: Priority,
        mut stack: Vec<u8>,
        entry_point: *const (),
    ) -> Self {
        let mut stack_ptr = stack.as_ptr() as usize; // Stack grows downwards

        let stack_unused = Self::setup_stack(stack_ptr, &mut stack, entry_point);
        stack_ptr += stack_unused; // Move stack pointer to the end of the stack

        // FIXME: Stack doesn't have guard page

        Self {
            id: ThreadId::new(),
            root_proc,
            priority,
            stack: Some(stack),
            last_stack_ptr: Box::pin(stack_ptr),
            link: Link::default(),
        }
    }

    /// Setup the stack and move stack pointer to the end of the stack.
    fn setup_stack(stack_ptr: usize, stack: &mut [u8], entry_point: *const ()) -> usize {
        // Can be used to detect stack overflow
        stack.fill(0xCD);

        let mut stack_bottom = stack.len();
        assert!(
            stack_bottom >= MINIMUM_LEFTOVER_STACK + 19 * size_of::<usize>(),
            "Stack too small"
        );

        // TODO: Write a custom thread_end function at the end of the stack

        // Push the return address
        let entry_point_bytes = (entry_point as usize).to_ne_bytes();
        stack[stack_bottom - size_of::<usize>()..stack_bottom].copy_from_slice(&entry_point_bytes);
        stack_bottom -= size_of::<usize>();

        // Push the thread registers
        let rsp = u64::try_from(stack_ptr).unwrap();
        let thread_regs = ThreadRegisters {
            rflags: (RFlags::IOPL_LOW | RFlags::INTERRUPT_FLAG).bits(),
            rsp,
            rbp: rsp,
            rip: entry_point as u64,
            ..ThreadRegisters::default()
        };
        debug_assert_eq!(size_of::<ThreadRegisters>(), 144);
        let thread_regs_bytes =
            unsafe { core::mem::transmute::<ThreadRegisters, [u8; 144]>(thread_regs) };
        stack[stack_bottom - 144..stack_bottom].copy_from_slice(&thread_regs_bytes);
        stack_bottom -= 144;

        debug_assert!(stack_bottom >= MINIMUM_LEFTOVER_STACK);
        stack_bottom
    }

    #[must_use]
    pub(super) fn new_stub(root_proc: Arc<Process>) -> Self {
        Self {
            id: ThreadId::new(),
            root_proc,
            priority: Priority::Low,
            stack: None,
            last_stack_ptr: Box::pin(0),
            link: Link::default(),
        }
    }

    /// Changes the priority of the thread.
    ///
    /// ## Safety
    ///
    /// This function should only be called on a currently active thread,
    /// as queues in the scheduler are sorted by priority.
    pub(super) unsafe fn set_priority(&mut self, priority: Priority) {
        self.priority = priority;
    }

    // pub(super) fn allocate_stack(&mut self, size: usize) {
    //     if self.stack.is_some() {
    //         log::warn!("Thread stack already allocated");
    //     }
    //     self.stack = Some(alloc::vec![0; size]);
    // }

    #[must_use]
    #[inline]
    pub const fn id(&self) -> ThreadId {
        self.id
    }

    #[must_use]
    #[inline]
    pub const fn priority(&self) -> Priority {
        self.priority
    }

    #[must_use]
    #[inline]
    pub fn process(&self) -> Arc<Process> {
        self.root_proc.clone()
    }

    #[must_use]
    #[inline]
    pub fn stack(&self) -> Option<&[u8]> {
        self.stack.as_ref().map(core::convert::AsRef::as_ref)
    }

    #[must_use]
    #[inline]
    pub fn stack_mut(&mut self) -> Option<&mut [u8]> {
        self.stack.as_mut().map(core::convert::AsMut::as_mut)
    }

    #[must_use]
    #[inline]
    /// Returns the value of the last stack pointer.
    pub fn last_stack_ptr(&self) -> *const usize {
        *self.last_stack_ptr.as_ref() as *const usize
    }

    #[must_use]
    #[inline]
    /// Returns a mutable pointer to the last stack pointer.
    pub fn last_stack_ptr_mut(&mut self) -> *mut usize {
        self.last_stack_ptr.as_mut().get_mut()
    }
}

static TID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ThreadId(u64);

impl core::ops::Deref for ThreadId {
    type Target = u64;

    fn deref(&self) -> &u64 {
        &self.0
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadId {
    pub fn new() -> Self {
        Self(TID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
    rsp: u64,
    rbx: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
    rflags: u64,
    rip: u64,
    // FIXME: SSE/FPU registers?
}
