use core::{
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
};

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use hyperdrive::queues::mpsc::{Link, Queueable};

use super::{super::Process, priority::Priority};

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

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        unsafe { Pin::new(Box::from_raw(ptr.as_ptr())) }
    }

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
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
    pub fn new(root_process: Arc<Process>, priority: Priority, stack: Option<Vec<u8>>) -> Self {
        Self {
            id: ThreadId::new(),
            root_proc: root_process,
            priority,
            stack,
            // Will be overwritten before being used.
            last_stack_ptr: Box::pin(0),
            link: Link::default(),
        }
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

    pub(super) fn allocate_stack(&mut self, size: usize) {
        if self.stack.is_some() {
            log::warn!("Thread stack already allocated");
        }
        self.stack = Some(alloc::vec![0; size]);
    }

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
    pub fn last_stack_ptr(&self) -> *const usize {
        core::ptr::from_ref(self.last_stack_ptr.as_ref().get_ref())
    }

    #[must_use]
    #[inline]
    pub fn last_stack_ptr_mut(&mut self) -> *mut usize {
        core::ptr::from_mut(self.last_stack_ptr.as_mut().get_mut())
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadRegisters {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
    // FIXME: SSE/FPU registers?
}
