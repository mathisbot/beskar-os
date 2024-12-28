use core::sync::atomic::{AtomicU64, Ordering};

use alloc::{sync::Arc, vec::Vec};

use super::{super::Process, priority::Priority};

#[derive(Clone)]
pub struct Thread {
    id: ThreadId,
    root_proc: Arc<Process>,
    priority: Priority,
    stack: Option<Vec<u8>>,
}

impl Thread {
    #[must_use]
    #[inline]
    pub fn new(root_process: Arc<Process>, priority: Priority) -> Self {
        Self {
            id: ThreadId::new(),
            root_proc: root_process,
            priority,
            stack: None,
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
    pub fn stack_ptr(&self) -> Option<&[u8]> {
        self.stack.as_ref().map(core::convert::AsRef::as_ref)
    }

    #[must_use]
    #[inline]
    pub fn stack_mut(&mut self) -> Option<&mut [u8]> {
        self.stack.as_mut().map(core::convert::AsMut::as_mut)
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
