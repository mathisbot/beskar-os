use core::{
    mem::offset_of,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
};

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use beskar_core::arch::{
    commons::paging::{CacheFlush, Flags, FrameAllocator, M4KiB, Mapper, MemSize, Page},
    x86_64::{instructions::STACK_DEBUG_INSTR, registers::Rflags, userspace::Ring},
};
use hyperdrive::{
    once::Once,
    queues::mpsc::{Link, Queueable},
};

use crate::mem::{frame_alloc, page_alloc};

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
    stack: Option<ThreadStacks>,
    /// Keeps track of where the stack is.
    ///
    /// The usize is the last stack pointer.
    /// The reason it is a pinned `Box` is so that we can easily get a reference to it
    /// and update it when switching contexts.
    pub(super) last_stack_ptr: Pin<Box<*mut u8>>,

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
        unsafe { ptr.byte_add(offset_of!(Self, link)) }.cast()
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
            last_stack_ptr: Box::pin(core::ptr::null_mut()),
            link: Link::default(),
        }
    }

    #[must_use]
    /// Create a new thread with a given entry point and stack.
    pub fn new(
        root_proc: Arc<Process>,
        priority: Priority,
        mut stack: Vec<u8>,
        entry_point: extern "C" fn(),
    ) -> Self {
        let mut stack_ptr = stack.as_mut_ptr(); // Stack grows downwards

        let stack_unused = Self::setup_stack(stack_ptr, &mut stack, entry_point);
        stack_ptr = unsafe { stack_ptr.byte_add(stack_unused) }; // Move stack pointer to the end of the stack

        // FIXME: Stack doesn't have guard page

        Self {
            id: ThreadId::new(),
            root_proc,
            priority,
            stack: Some(ThreadStacks::new(stack)),
            last_stack_ptr: Box::pin(stack_ptr),
            link: Link::default(),
        }
    }

    #[must_use]
    /// Create a new thread with the given stack, and the root process' binary.
    ///
    /// # Panics
    ///
    /// Panics if the root process does not have a binary.
    pub fn new_from_binary(root_proc: Arc<Process>, priority: Priority, stack: Vec<u8>) -> Self {
        assert!(
            root_proc.binary_data.is_some(),
            "Root process has no binary"
        );

        let trampoline = match root_proc.kind.ring() {
            Ring::User => user_trampoline,
            Ring::Kernel => todo!("Ring0 binary threads"),
        };

        Self::new(root_proc, priority, stack, trampoline)
    }

    /// Setup the stack and move stack pointer to the end of the stack.
    fn setup_stack(stack_ptr: *mut u8, stack: &mut [u8], entry_point: extern "C" fn()) -> usize {
        // Can be used to detect stack overflow
        #[cfg(debug_assertions)]
        stack.fill(STACK_DEBUG_INSTR);

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
        let thread_regs = ThreadRegisters {
            rflags: (Rflags::IOPL_LOW | Rflags::IF),
            rbp: stack_ptr as u64,
            rip: entry_point as u64,
            ..ThreadRegisters::default()
        };
        let thread_regs_bytes = unsafe {
            core::mem::transmute::<ThreadRegisters, [u8; size_of::<ThreadRegisters>()]>(thread_regs)
        };
        stack[stack_bottom - size_of::<ThreadRegisters>()..stack_bottom]
            .copy_from_slice(&thread_regs_bytes);
        stack_bottom -= size_of::<ThreadRegisters>();

        debug_assert!(stack_bottom >= MINIMUM_LEFTOVER_STACK);
        stack_bottom
    }

    #[must_use]
    pub(super) fn new_stub(root_proc: Arc<Process>) -> Self {
        Self {
            id: ThreadId::new(),
            root_proc,
            priority: Priority::Null,
            stack: None,
            last_stack_ptr: Box::pin(core::ptr::null_mut()),
            link: Link::default(),
        }
    }

    /// Changes the priority of the thread.
    ///
    /// ## Safety
    ///
    /// This function should only be called on a currently active thread,
    /// as queues in the scheduler are sorted by priority.
    pub(super) const unsafe fn set_priority(&mut self, priority: Priority) {
        self.priority = priority;
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
    /// Returns the value of the last stack pointer.
    pub fn last_stack_ptr(&self) -> *const u8 {
        *self.last_stack_ptr.as_ref()
    }

    #[must_use]
    #[inline]
    /// Returns a mutable pointer to the last stack pointer.
    pub fn last_stack_ptr_mut(&mut self) -> *mut *mut u8 {
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
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self(TID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    #[must_use]
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[repr(C)]
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
    rbx: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
    rflags: u64,
    rip: u64,
}

/// Trampoline function to load the binary and call the entry point.
///
/// ## Warning
///
/// This function should not be called directly, but rather be used
/// as an entry point for threads.
extern "C" fn user_trampoline() {
    let root_proc = super::current_process();

    // Load the binary into the process' address space.
    let entry_point = root_proc.load_binary();

    // Allocate a user stack
    let rsp = super::get_scheduler()
        .current_thread
        .with_locked(|t| t.stack.as_mut().map(|ts| ts.allocate_user(4 * M4KiB::SIZE)))
        .expect("Thread stack not found");

    unsafe { crate::arch::userspace::enter_usermode(entry_point, rsp) };

    // TODO: After tests, remove this
    unreachable!("Thread trampline returned");
}

struct ThreadStacks {
    /// The stack allocated in the kernel's address space.
    ///
    /// This can be the only stack used (ring0 processes) or
    /// only used by the trampoline function (ring3 processes).
    _kernel: Vec<u8>,
    /// Page in the process' address space where the stack starts.
    user_start_page: Once<Page>,
    /// Size of the user stack in bytes.
    user_size: Once<u64>,
}

impl ThreadStacks {
    #[must_use]
    #[inline]
    pub const fn new(stack: Vec<u8>) -> Self {
        Self {
            _kernel: stack,
            user_start_page: Once::uninit(),
            user_size: Once::uninit(),
        }
    }

    pub fn allocate_user(&self, size: u64) -> *mut u8 {
        // FIXME: Use the process' page allocator to allocate the stack.
        // FIXME: Allocate guarded ?
        let page_range = page_alloc::with_page_allocator(|palloc| {
            palloc.allocate_pages::<M4KiB>(size.div_ceil(M4KiB::SIZE))
        })
        .unwrap();

        frame_alloc::with_frame_allocator(|fralloc| {
            super::current_process()
                .address_space()
                .with_page_table(|pt| {
                    let frame = fralloc.allocate_frame().unwrap();
                    let flags = Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE;
                    for page in page_range {
                        pt.map(page, frame, flags, fralloc).flush();
                    }
                });
        });

        // FIXME: Even if the stack is already allocated, the above allocations still happen.
        self.user_start_page.call_once(|| page_range.start);
        self.user_size.call_once(|| page_range.len() * M4KiB::SIZE);

        // Return the stack TOP
        let stack_bottom = page_range.start.start_address().as_mut_ptr::<u8>();
        #[cfg(debug_assertions)]
        unsafe {
            stack_bottom.write_bytes(STACK_DEBUG_INSTR, size.try_into().unwrap());
        }
        unsafe { stack_bottom.byte_add(size.try_into().unwrap()) }
    }
}

impl Drop for ThreadStacks {
    fn drop(&mut self) {
        if let Some(&start_page) = self.user_start_page.get() {
            let &size = self.user_size.get().unwrap();

            let end_page = Page::<M4KiB>::containing_address(start_page.start_address() + size - 1);

            frame_alloc::with_frame_allocator(|fralloc| {
                super::current_process()
                    .address_space()
                    .with_page_table(|pt| {
                        for page in Page::range_inclusive(start_page, end_page) {
                            let (frame, tlb) = pt.unmap(page).unwrap();
                            fralloc.deallocate_frame(frame);
                            tlb.flush();
                        }
                    });
            });

            // FIXME: Use the process' page allocator to allocate the stack.
            page_alloc::with_page_allocator(|palloc| {
                palloc.free_pages(Page::range_inclusive(start_page, end_page));
            });
        }
    }
}
