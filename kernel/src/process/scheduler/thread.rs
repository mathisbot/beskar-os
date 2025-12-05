use crate::{
    arch::context::ThreadRegisters,
    mem::frame_alloc,
    process::binary::{Binary, BinaryType, LoadedBinary},
    storage::vfs,
};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use beskar_core::arch::{
    VirtAddr,
    paging::{CacheFlush, FrameAllocator, M4KiB, Mapper, MemSize, PageRangeInclusive},
};
use beskar_hal::{instructions::STACK_DEBUG_INSTR, paging::page_table::Flags};
use core::{
    mem::offset_of,
    pin::Pin,
    ptr::NonNull,
    sync::atomic::{AtomicU64, Ordering},
};
use hyperdrive::{
    once::Once,
    queues::mpsc::{Link, Queueable},
};
use storage::fs::Path;

use super::{super::Process, priority::Priority};

/// The minimum amount of stack space that must be left unused on thread creation.
const MINIMUM_LEFTOVER_STACK: usize = 0x100; // 256 bytes

/// Thread statistics
#[derive(Debug, Clone, Copy)]
pub struct ThreadStats {
    pub cpu_time_ms: u64,
    pub wake_time: beskar_core::time::Instant,
}

impl ThreadStats {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            cpu_time_ms: 0,
            wake_time: beskar_core::time::Instant::ZERO,
        }
    }
}

impl Default for ThreadStats {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Thread {
    /// The unique identifier of the thread.
    id: ThreadId,
    /// The process that this thread belongs to.
    root_proc: Arc<Process>,
    /// The priority of the thread.
    priority: Priority,
    /// The state of the thread.
    state: ThreadState,
    /// Used to keep ownership of the stacks when needed.
    stack: Option<ThreadStacks>,
    /// Keeps track of where the stack pointer is.
    last_stack_ptr: *mut u8,
    /// Thread Local Storage
    tls: Once<Tls>,
    /// Thread statistics for scheduling
    stats: ThreadStats,

    /// Link to the next thread in the queue.
    link: Link<Self>,
}

impl Unpin for Thread {}

impl PartialEq for Thread {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Thread {}

impl PartialOrd for Thread {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Thread {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

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
    pub(in super::super) fn new_kernel(kernel_process: Arc<Process>) -> Self {
        Self {
            id: ThreadId::new(),
            root_proc: kernel_process,
            priority: Priority::High,
            state: ThreadState::Running,
            stack: None,
            // Will be overwritten before being used.
            last_stack_ptr: core::ptr::null_mut(),
            link: Link::new(),
            tls: Once::uninit(),
            stats: ThreadStats::new(),
        }
    }

    #[must_use]
    /// Create a new thread with a given entry point and stack.
    pub fn new(
        root_proc: Arc<Process>,
        priority: Priority,
        mut stack: Vec<u8>,
        entry_point: extern "C" fn() -> !,
    ) -> Self {
        let mut stack_ptr = stack.as_mut_ptr(); // Stack grows downwards

        let stack_unused = Self::setup_stack(stack_ptr, &mut stack, entry_point);
        stack_ptr = unsafe { stack_ptr.byte_add(stack_unused) }; // Move stack pointer to the end of the stack

        Self {
            id: ThreadId::new(),
            root_proc,
            priority,
            state: ThreadState::Ready,
            stack: Some(ThreadStacks::new(stack)),
            last_stack_ptr: stack_ptr,
            link: Link::new(),
            tls: Once::uninit(),
            stats: ThreadStats::new(),
        }
    }

    /// Setup the stack and move stack pointer to the end of the stack.
    fn setup_stack(
        stack_ptr: *mut u8,
        stack: &mut [u8],
        entry_point: extern "C" fn() -> !,
    ) -> usize {
        // Can be used to detect stack overflow
        #[cfg(debug_assertions)]
        stack.fill(STACK_DEBUG_INSTR);

        let mut stack_bottom = stack.len();
        assert!(
            stack_bottom
                >= MINIMUM_LEFTOVER_STACK + size_of::<ThreadRegisters>() + size_of::<usize>(),
            "Stack too small"
        );

        // Push the return address
        let entry_point_bytes = (entry_point as usize).to_ne_bytes();
        stack[stack_bottom - size_of::<usize>()..stack_bottom].copy_from_slice(&entry_point_bytes);
        stack_bottom -= size_of::<usize>();

        // Push the thread registers
        let thread_regs = ThreadRegisters::new(entry_point, stack_ptr);
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
    pub(super) const fn new_stub(root_proc: Arc<Process>) -> Self {
        Self {
            id: ThreadId(0),
            root_proc,
            priority: Priority::Low,
            state: ThreadState::Ready,
            stack: None,
            last_stack_ptr: core::ptr::null_mut(),
            link: Link::new(),
            tls: Once::uninit(),
            stats: ThreadStats::new(),
        }
    }

    #[inline]
    /// Changes the state of the thread.
    ///
    /// # Safety
    ///
    /// This function should only be called on a currently active thread.
    pub(super) const unsafe fn set_state(&mut self, state: ThreadState) {
        self.state = state;
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
    pub const fn state(&self) -> ThreadState {
        self.state
    }

    #[must_use]
    #[inline]
    pub const fn stats(&self) -> &ThreadStats {
        &self.stats
    }

    #[must_use]
    #[inline]
    pub const fn stats_mut(&mut self) -> &mut ThreadStats {
        &mut self.stats
    }

    #[must_use]
    #[inline]
    pub fn process(&self) -> Arc<Process> {
        self.root_proc.clone()
    }

    #[must_use]
    #[inline]
    /// Returns the value of the last stack pointer.
    pub const fn last_stack_ptr(&self) -> *const u8 {
        self.last_stack_ptr
    }

    #[must_use]
    #[inline]
    /// Returns a mutable pointer to the last stack pointer.
    pub const fn last_stack_ptr_mut(&mut self) -> *mut *mut u8 {
        &raw mut self.last_stack_ptr
    }

    #[must_use]
    #[inline]
    /// Returns the thread local storage of the thread.
    pub fn tls(&self) -> Option<Tls> {
        self.tls.get().copied()
    }

    #[must_use]
    /// Get a snapshot of the thread's state.
    pub fn snapshot(&self) -> ThreadSnapshot {
        let kst = self.stack.as_ref().map(ThreadStacks::kernel_stack_top);
        ThreadSnapshot::new(self.id, kst)
    }
}

#[derive(Debug, Clone, Copy)]
/// Represents a snapshot of a thread's state.
pub struct ThreadSnapshot {
    /// The unique identifier of the thread.
    id: ThreadId,
    /// RSP0.
    kernel_stack_top: Option<NonNull<u8>>,
}

impl ThreadSnapshot {
    #[must_use]
    #[inline]
    pub(super) const fn new(id: ThreadId, kst: Option<NonNull<u8>>) -> Self {
        Self {
            id,
            kernel_stack_top: kst,
        }
    }

    #[must_use]
    #[inline]
    pub const fn id(&self) -> ThreadId {
        self.id
    }

    #[must_use]
    #[inline]
    pub const fn kernel_stack_top(&self) -> Option<NonNull<u8>> {
        self.kernel_stack_top
    }
}

static TID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
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

fn thread_load_binary(path: Path) -> LoadedBinary {
    let curr_proc = super::current_process();
    let handle = vfs().open(path).unwrap();

    let file_info = vfs().metadata(path).unwrap();

    let page_range = curr_proc
        .address_space()
        .alloc_map::<M4KiB>(file_info.size(), Flags::PRESENT | Flags::WRITABLE)
        .unwrap();

    let input_buffer = unsafe {
        core::slice::from_raw_parts_mut(
            page_range.start().start_address().as_mut_ptr::<u8>(),
            file_info.size(),
        )
    };
    let input_bytes = vfs().read(handle, input_buffer, 0).unwrap();
    assert_eq!(input_bytes, input_buffer.len());

    vfs().close(handle).unwrap();

    let binary = Binary::new(input_buffer, BinaryType::Elf);
    let loaded_binary = binary.load().unwrap();

    // Safety: Binary has been laoded, input bytes can be freed.
    unsafe { curr_proc.address_space().unmap_free(page_range) };

    loaded_binary
}

/// Trampoline function to load the binary and call the entry point.
///
/// # Warning
///
/// This function should not be called directly, but rather be used
/// as an entry point for threads.
pub extern "C" fn user_trampoline() -> ! {
    let root_proc = super::current_process();
    let loaded_binary = thread_load_binary(root_proc.binary().unwrap());

    // Allocate a user stack
    let rsp = super::with_scheduler(|scheduler| {
        scheduler.current.with_locked(|thread| {
            thread.stack.as_mut().map(|ts| {
                ts.allocate_all(4 * M4KiB::SIZE);
                ts.user_stack_top().unwrap()
            })
        })
    })
    .expect("Current thread stack allocation failed")
    .as_ptr();

    if let Some(tlst) = loaded_binary.tls_template() {
        let tls_size = tlst.mem_size();

        let pages = root_proc
            .address_space()
            .alloc_map::<M4KiB>(
                usize::try_from(tls_size).unwrap(),
                Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
            )
            .unwrap();
        let tls_vaddr = pages.start().start_address();

        // Copy TLS initialization image from binary
        unsafe {
            tls_vaddr.as_mut_ptr::<u8>().copy_from_nonoverlapping(
                tlst.start().as_ptr(),
                tlst.file_size().try_into().unwrap(),
            );
        }
        // Zero the rest of the TLS area
        unsafe {
            tls_vaddr
                .as_mut_ptr::<u8>()
                .byte_add(tlst.file_size().try_into().unwrap())
                .write_bytes(0, (tlst.mem_size() - tlst.file_size()).try_into().unwrap());
        }

        let tls = Tls {
            addr: tls_vaddr,
            size: tls_size,
        };

        // Locking the scheduler's current thread is a bit ugly, but it is better than force locking it
        // (as otherwise the scheduler could get stuck on `Once::get`).
        super::with_scheduler(|scheduler| {
            scheduler.current.with_locked(|thread| {
                thread.tls.call_once(|| tls);
            });
        });
        crate::arch::locals::store_thread_locals(tls);
    }

    drop(root_proc); // Decrease the reference count of the process
    unsafe { crate::arch::userspace::enter_usermode(loaded_binary.entry_point(), rsp) };
}

struct ThreadStacks {
    /// The stack allocated in the kernel's address space.
    ///
    /// This can be the only stack used (ring0 processes) or
    /// only used by the trampoline function (ring3 processes).
    kernel: Vec<u8>,
    /// Page range in the process' address space of the stack.
    user_pages: Once<PageRangeInclusive>,
}

impl ThreadStacks {
    const STACK_ALIGNMENT: u64 = 16;

    #[must_use]
    #[inline]
    pub const fn new(stack: Vec<u8>) -> Self {
        Self {
            kernel: stack,
            user_pages: Once::uninit(),
        }
    }

    pub fn allocate_all(&self, size: u64) {
        self.allocate_user(size);
    }

    pub fn allocate_user(&self, size: u64) {
        let flags = Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE;
        self.user_pages.call_once(|| Self::allocate(size, flags));
    }

    #[must_use]
    pub fn user_stack_top(&self) -> Option<NonNull<u8>> {
        self.user_pages
            .get()
            .map(|r| r.start().start_address() + r.size())
            .and_then(|p| NonNull::new(p.align_down(Self::STACK_ALIGNMENT).as_mut_ptr()))
    }

    #[must_use]
    pub fn kernel_stack_top(&self) -> NonNull<u8> {
        let stack_start = self.kernel.as_ptr() as usize;
        let stack_vaddr = VirtAddr::new(u64::try_from(stack_start).unwrap());
        let stack_end = stack_vaddr + u64::try_from(self.kernel.len()).unwrap();
        unsafe { NonNull::new_unchecked(stack_end.align_down(Self::STACK_ALIGNMENT).as_mut_ptr()) }
    }

    fn allocate(size: u64, flags: Flags) -> PageRangeInclusive {
        assert!(size >= Self::STACK_ALIGNMENT);

        let (_guard_start, page_range, _guard_end) = super::current_process()
            .address_space()
            .with_pgalloc(|palloc| palloc.allocate_guarded(size.div_ceil(M4KiB::SIZE)))
            .unwrap();

        frame_alloc::with_frame_allocator(|fralloc| {
            super::current_process()
                .address_space()
                .with_page_table(|pt| {
                    for page in page_range {
                        let frame = fralloc.allocate_frame().unwrap();
                        pt.map(page, frame, flags, fralloc).flush();
                    }
                });
        });

        #[cfg(debug_assertions)]
        unsafe {
            let stack_bottom = page_range.start().start_address();
            let size = page_range.size();
            stack_bottom
                .as_mut_ptr::<u8>()
                .write_bytes(STACK_DEBUG_INSTR, size.try_into().unwrap());
        }

        page_range
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Tls {
    /// The address of the TLS area.
    addr: VirtAddr,
    /// The size of the TLS area.
    size: u64,
}

impl Tls {
    #[must_use]
    #[inline]
    pub const fn addr(&self) -> VirtAddr {
        self.addr
    }

    #[must_use]
    #[inline]
    pub const fn size(&self) -> u64 {
        self.size
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The state of a thread.
pub enum ThreadState {
    /// The thread is running.
    Running,
    /// The thread is ready to run.
    Ready,
    /// The thread is waiting for an event.
    Sleeping,
}
