use crate::{
    arch::{context::ThreadRegisters, fpu::FpuState},
    mem::vmm,
    process::{
        ProcessId,
        binary::{Binary, BinaryType, LoadedBinary},
    },
    storage::vfs,
};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use beskar_core::arch::{
    Alignment, VirtAddr,
    paging::{M4KiB, MemSize, PageRangeInclusive},
};
use beskar_core::process::ThreadStartBlock;
#[cfg(debug_assertions)]
use beskar_hal::instructions::STACK_DEBUG_INSTR;
use beskar_hal::paging::page_table::Flags;
use core::{
    mem::offset_of,
    ptr::NonNull,
    sync::atomic::{AtomicPtr, AtomicU64, Ordering},
};
use hyperdrive::{
    once::Once,
    queues::mpsc::{Link, Queueable},
};
use storage::fs::Path;
use wait::WakeResult;

use super::{super::Process, priority::Priority};

/// The minimum amount of stack space that must be left unused on thread creation.
const MINIMUM_LEFTOVER_STACK: usize = 0x100; // 256 bytes

/// Default stack size used by [`ThreadBuilder`] when no explicit size is set.
const DEFAULT_THREAD_STACK_SIZE: usize = 64 * 1024;

/// Default user stack size for user process startup.
const DEFAULT_USER_STACK_SIZE: u64 = 16 * M4KiB::SIZE;

type ThreadEntry = extern "C" fn() -> !;
type ThreadEntryArgs = extern "C" fn(usize) -> !;

#[derive(Debug, Clone, Copy)]
struct ThreadStart {
    entry: ThreadEntryArgs,
    arg: usize,
}

impl ThreadStart {
    #[must_use]
    #[inline]
    pub fn new(entry: ThreadEntry) -> Self {
        extern "C" fn trampoline(arg: usize) -> ! {
            let entry = unsafe { core::mem::transmute::<*const (), ThreadEntry>(arg as _) };
            entry()
        }

        Self {
            entry: trampoline,
            arg: (entry as *const ()).addr(),
        }
    }

    #[must_use]
    #[inline]
    pub const fn new_arg(entry: ThreadEntryArgs, arg: usize) -> Self {
        Self { entry, arg }
    }
}

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
            wake_time: beskar_core::time::Instant::ORIGIN,
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
    /// Armed token for the current blocking operation.
    wait_token: u64,
    /// Last wake result delivered by the wait subsystem.
    wake_result: WakeResult,
    /// Used to keep ownership of the stacks when needed.
    stack: Option<ThreadStacks>,
    /// Keeps track of where the stack pointer is.
    last_stack_ptr: AtomicPtr<u8>,
    /// Thread statistics for scheduling
    stats: ThreadStats,
    /// FPU/SSE state for lazy context switching
    fpu_state: FpuState,
    /// Userspace FS base
    user_fs_base: VirtAddr,

    /// Link to the next thread in the queue.
    link: Link<Self>,
}

impl PartialEq for Thread {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Thread {}

impl Queueable for Thread {
    type Handle = Box<Self>;

    unsafe fn capture(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        unsafe { Box::from_raw(ptr.as_ptr()) }
    }

    fn release(r: Self::Handle) -> core::ptr::NonNull<Self> {
        let ptr = Box::into_raw(r);
        unsafe { core::ptr::NonNull::new_unchecked(ptr) }
    }

    unsafe fn get_link(ptr: core::ptr::NonNull<Self>) -> core::ptr::NonNull<Link<Self>> {
        unsafe { ptr.byte_add(offset_of!(Self, link)) }.cast()
    }
}

impl Thread {
    #[must_use]
    #[inline]
    /// Creates a thread builder with sensible defaults.
    pub fn builder(root_proc: Arc<Process>, entry_point: ThreadEntry) -> ThreadBuilder {
        ThreadBuilder::new(root_proc, entry_point)
    }

    #[must_use]
    #[inline]
    /// Creates a thread builder for an entry point that receives an argument.
    pub fn builder_with_arg(
        root_proc: Arc<Process>,
        entry_point: ThreadEntryArgs,
        arg: usize,
    ) -> ThreadBuilder {
        ThreadBuilder::with_arg(root_proc, entry_point, arg)
    }

    #[must_use]
    #[inline]
    pub(in super::super) fn new_kernel(kernel_process: Arc<Process>) -> Self {
        Self {
            id: ThreadId::new(),
            root_proc: kernel_process,
            priority: Priority::High,
            state: ThreadState::Running,
            wait_token: 0,
            wake_result: WakeResult::none(),
            stack: None,
            // Will be overwritten before being used.
            last_stack_ptr: AtomicPtr::new(core::ptr::null_mut()),
            link: Link::new(),
            stats: ThreadStats::new(),
            fpu_state: FpuState::new(0),
            user_fs_base: VirtAddr::ZERO,
        }
    }

    #[must_use]
    /// Create a new thread with a given entry point and stack.
    pub fn new(
        root_proc: Arc<Process>,
        priority: Priority,
        stack: Vec<u8>,
        entry_point: ThreadEntry,
    ) -> Self {
        Self::from_parts(
            root_proc,
            priority,
            ThreadStack::Heap(stack),
            ThreadStart::new(entry_point),
        )
    }

    #[must_use]
    /// Create a new thread with a given entry point, argument and stack.
    pub fn new_with_arg(
        root_proc: Arc<Process>,
        priority: Priority,
        stack: Vec<u8>,
        entry_point: ThreadEntryArgs,
        arg: usize,
    ) -> Self {
        Self::from_parts(
            root_proc,
            priority,
            ThreadStack::Heap(stack),
            ThreadStart::new_arg(entry_point, arg),
        )
    }

    #[must_use]
    fn from_parts(
        root_proc: Arc<Process>,
        priority: Priority,
        mut stack: ThreadStack,
        start: ThreadStart,
    ) -> Self {
        let mut stack_ptr = stack.base_ptr();

        let stack_unused = Self::setup_stack(stack_ptr, stack.as_mut_slice(), start);
        stack_ptr = unsafe { stack_ptr.byte_add(stack_unused) }; // Move stack pointer to the end of the stack

        let xcr0 = beskar_hal::registers::XCr0::read();

        Self {
            id: ThreadId::new(),
            root_proc,
            priority,
            state: ThreadState::Runnable,
            wait_token: 0,
            wake_result: WakeResult::none(),
            stack: Some(ThreadStacks::new(stack)),
            last_stack_ptr: AtomicPtr::new(stack_ptr),
            link: Link::new(),
            stats: ThreadStats::new(),
            fpu_state: FpuState::new(xcr0),
            user_fs_base: VirtAddr::ZERO,
        }
    }

    /// Setup the stack and move stack pointer to the end of the stack.
    fn setup_stack(stack_ptr: *mut u8, stack: &mut [u8], start: ThreadStart) -> usize {
        // Can be used to detect stack overflow
        #[cfg(debug_assertions)]
        stack.fill(STACK_DEBUG_INSTR);

        let mut stack_bottom = stack.len();
        assert!(
            stack_bottom >= MINIMUM_LEFTOVER_STACK + size_of::<ThreadRegisters>(),
            "Stack too small"
        );

        // Push the thread registers
        let thread_regs = ThreadRegisters::new(start.entry, start.arg, stack_ptr);
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
            state: ThreadState::Runnable,
            wait_token: 0,
            wake_result: WakeResult::none(),
            stack: None,
            last_stack_ptr: AtomicPtr::new(core::ptr::null_mut()),
            link: Link::new(),
            stats: ThreadStats::new(),
            fpu_state: FpuState::new(0),
            user_fs_base: VirtAddr::ZERO,
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
    pub const fn wait_token(&self) -> u64 {
        self.wait_token
    }

    #[inline]
    pub(super) const fn arm_wait(&mut self, token: u64) {
        self.wait_token = token;
        self.wake_result = WakeResult::none();
    }

    #[inline]
    pub(super) const fn disarm_wait(&mut self) {
        self.wait_token = 0;
    }

    #[inline]
    pub(super) const fn set_wake_result(&mut self, wake_result: WakeResult) {
        self.wake_result = wake_result;
    }

    #[must_use]
    #[inline]
    pub const fn wake_result(&self) -> WakeResult {
        self.wake_result
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
    pub fn last_stack_ptr(&self) -> *const u8 {
        self.last_stack_ptr.load(Ordering::Acquire)
    }

    #[must_use]
    #[inline]
    /// Returns a mutable pointer to the last stack pointer.
    ///
    /// # Safety
    ///
    /// The caller must use atomic operations to read/write the pointer.
    pub const unsafe fn last_stack_ptr_mut(&mut self) -> *mut *mut u8 {
        self.last_stack_ptr.as_ptr()
    }

    #[must_use]
    #[inline]
    /// Returns a reference to the thread's FPU state.
    pub const fn fpu_state(&self) -> &FpuState {
        &self.fpu_state
    }

    #[must_use]
    #[inline]
    /// Returns a mutable reference to the thread's FPU state.
    pub const fn fpu_state_mut(&mut self) -> &mut FpuState {
        &mut self.fpu_state
    }

    #[must_use]
    #[inline]
    pub const fn user_fs_base(&self) -> VirtAddr {
        self.user_fs_base
    }

    #[inline]
    pub(super) const fn set_user_fs_base(&mut self, value: VirtAddr) {
        self.user_fs_base = value;
    }

    #[must_use]
    /// Get a snapshot of the thread's state.
    pub fn snapshot(&self) -> ThreadSnapshot {
        let kst = self.stack.as_ref().map(ThreadStacks::kernel_stack_top);
        let fpu = self.fpu_state();
        ThreadSnapshot::new(self.id, kst, fpu)
    }
}

enum ThreadStack {
    Heap(Vec<u8>),
    Pages(PageRangeInclusive<M4KiB>),
}

impl ThreadStack {
    #[must_use]
    #[inline]
    fn heap(size: usize) -> Self {
        Self::Heap(alloc::vec![0; size])
    }

    #[must_use]
    #[inline]
    fn pool(size: usize, flags: Flags) -> Self {
        let pages = vmm::kernel::alloc_map(size, flags).unwrap();
        Self::Pages(pages)
    }

    #[must_use]
    #[inline]
    const fn base_ptr(&self) -> *mut u8 {
        match self {
            Self::Heap(stack) => stack.as_ptr().cast_mut(),
            Self::Pages(pages) => pages.start().start_address().as_mut_ptr(),
        }
    }

    #[must_use]
    #[inline]
    fn len(&self) -> usize {
        match self {
            Self::Heap(stack) => stack.len(),
            Self::Pages(pages) => usize::try_from(pages.size()).unwrap(),
        }
    }

    #[must_use]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.base_ptr(), self.len()) }
    }

    #[must_use]
    fn top_aligned(&self, alignment: Alignment) -> NonNull<u8> {
        let start = VirtAddr::from_ptr(self.base_ptr());
        let end = start + u64::try_from(self.len()).unwrap();
        unsafe { NonNull::new_unchecked(end.aligned_down(alignment).as_mut_ptr()) }
    }
}

impl Drop for ThreadStack {
    fn drop(&mut self) {
        match self {
            Self::Heap(_) => {}
            Self::Pages(pages) => unsafe { vmm::kernel::unmap_free(*pages) },
        }
    }
}

pub struct ThreadBuilder {
    root_proc: Arc<Process>,
    priority: Priority,
    start: ThreadStart,
    stack: ThreadStack,
}

impl ThreadBuilder {
    #[must_use]
    #[inline]
    pub fn new(root_proc: Arc<Process>, entry_point: ThreadEntry) -> Self {
        Self::with_start(root_proc, ThreadStart::new(entry_point))
    }

    #[must_use]
    #[inline]
    pub fn with_arg(root_proc: Arc<Process>, entry_point: ThreadEntryArgs, arg: usize) -> Self {
        Self::with_start(root_proc, ThreadStart::new_arg(entry_point, arg))
    }

    #[must_use]
    #[inline]
    fn with_start(root_proc: Arc<Process>, start: ThreadStart) -> Self {
        Self {
            root_proc,
            priority: Priority::Normal,
            start,
            stack: ThreadStack::heap(DEFAULT_THREAD_STACK_SIZE),
        }
    }

    #[must_use]
    #[inline]
    pub const fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    #[must_use]
    #[inline]
    pub fn stack_heap(mut self, stack: Vec<u8>) -> Self {
        self.stack = ThreadStack::Heap(stack);
        self
    }

    #[must_use]
    #[inline]
    pub fn stack_pool(mut self, size: usize, flags: Flags) -> Self {
        self.stack = ThreadStack::pool(size, flags);
        self
    }

    #[must_use]
    pub fn build(self) -> Thread {
        Thread::from_parts(self.root_proc, self.priority, self.stack, self.start)
    }

    #[must_use]
    #[inline]
    pub fn build_boxed(self) -> Box<Thread> {
        Box::new(self.build())
    }

    #[inline]
    pub fn spawn(self) {
        super::spawn_thread(self.build_boxed());
    }
}

#[derive(Debug, Clone, Copy)]
/// Represents a snapshot of a thread's state.
pub struct ThreadSnapshot {
    /// The unique identifier of the thread.
    id: ThreadId,
    /// RSP0.
    kernel_stack_top: Option<NonNull<u8>>,
    /// FPU state pointer, used for lazy FPU context switching.
    fpu_state: *const FpuState,
}

impl ThreadSnapshot {
    #[must_use]
    #[inline]
    pub(super) const fn new(
        id: ThreadId,
        kst: Option<NonNull<u8>>,
        fpu_state: *const FpuState,
    ) -> Self {
        Self {
            id,
            kernel_stack_top: kst,
            fpu_state,
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

    #[must_use]
    #[inline]
    pub const fn fpu_state(&self) -> *const FpuState {
        self.fpu_state
    }
}

static TID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
pub struct ThreadId(u64);

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

fn thread_load_binary(path: Path, pid: ProcessId) -> LoadedBinary {
    let handle = vfs().open(pid.as_u64(), path).unwrap();

    let file_info = vfs().metadata(path).unwrap();

    let page_range = vmm::process_local::alloc_map::<M4KiB>(
        file_info.size(),
        Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
    )
    .unwrap();

    let input_buffer = unsafe {
        core::slice::from_raw_parts_mut(
            page_range.start().start_address().as_mut_ptr::<u8>(),
            file_info.size(),
        )
    };
    let input_bytes = vfs().read(pid.as_u64(), handle, input_buffer, 0).unwrap();
    assert_eq!(input_bytes, input_buffer.len());

    vfs().close(pid.as_u64(), handle).unwrap();

    let binary = Binary::new(input_buffer, BinaryType::Elf);
    let loaded_binary = binary.load().unwrap();

    // Safety: Binary has been laoded, input bytes can be freed.
    unsafe { vmm::process_local::unmap_free(page_range) };

    loaded_binary
}

/// User process startup entry point.
///
/// The argument is the desired user stack size in bytes.
pub extern "C" fn start_user_process(user_stack_size: usize) -> ! {
    let user_stack_size = if user_stack_size == 0 {
        DEFAULT_USER_STACK_SIZE
    } else {
        u64::try_from(user_stack_size).unwrap()
    };

    let root_proc = super::current_process();
    let loaded_binary = thread_load_binary(root_proc.binary().unwrap(), root_proc.pid());

    // Allocate a user stack
    let rsp = super::with_scheduler(|scheduler| {
        scheduler.current.with_locked(|thread| {
            thread.stack.as_mut().map(|ts| {
                ts.allocate_all(user_stack_size);
                ts.user_stack_top().unwrap()
            })
        })
    })
    .expect("Current thread stack allocation failed")
    .as_ptr();

    let start_block = ThreadStartBlock {
        start: loaded_binary.base_address(),
    };
    // Reserve space for the startup block on the user stack while preserving ABI alignment.
    let stack_reserve = size_of::<ThreadStartBlock>().next_multiple_of(16);
    let user_rsp = unsafe { rsp.byte_sub(stack_reserve) };
    #[expect(clippy::cast_ptr_alignment)]
    let start_block_ptr = user_rsp.cast::<ThreadStartBlock>();
    debug_assert!(start_block_ptr.is_aligned());
    unsafe { start_block_ptr.write(start_block) };

    drop(root_proc); // Decrease the reference count of the process
    debug_assert!(VirtAddr::from_ptr(user_rsp).is_aligned(Alignment::Align16));
    unsafe {
        crate::arch::userspace::enter_usermode(
            loaded_binary.entry_point(),
            user_rsp,
            start_block_ptr,
        )
    };
}

pub extern "C" fn start_user_thread(entry_point: extern "C" fn(*const ThreadStartBlock)) -> ! {
    // Allocate a user stack
    let rsp = super::with_scheduler(|scheduler| {
        scheduler.current.with_locked(|thread| {
            thread.stack.as_mut().map(|ts| {
                ts.allocate_all(DEFAULT_USER_STACK_SIZE);
                ts.user_stack_top().unwrap()
            })
        })
    })
    .expect("Current thread stack allocation failed")
    .as_ptr();

    unsafe { crate::arch::userspace::enter_usermode(entry_point, rsp, core::ptr::null()) };
}

struct ThreadStacks {
    /// The stack allocated in the kernel's address space.
    ///
    /// This can be the only stack used (ring0 processes) or
    /// only used by the trampoline function (ring3 processes).
    kernel: ThreadStack,
    /// Page range in the process' address space of the stack.
    user_pages: Once<PageRangeInclusive>,
}

impl ThreadStacks {
    const STACK_ALIGNMENT: Alignment = Alignment::Align16;

    #[must_use]
    #[inline]
    pub const fn new(stack: ThreadStack) -> Self {
        Self {
            kernel: stack,
            user_pages: Once::uninit(),
        }
    }

    pub fn allocate_all(&self, size: u64) {
        self.allocate_user(size);
    }

    pub fn allocate_user(&self, size: u64) {
        let flags = Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE | Flags::NO_EXECUTE;
        self.user_pages.call_once(|| Self::allocate(size, flags));
    }

    #[must_use]
    pub fn user_stack_top(&self) -> Option<NonNull<u8>> {
        self.user_pages
            .get()
            .map(|r| r.start().start_address() + r.size())
            .and_then(|p| NonNull::new(p.aligned_down(Self::STACK_ALIGNMENT).as_mut_ptr()))
    }

    #[must_use]
    pub fn kernel_stack_top(&self) -> NonNull<u8> {
        self.kernel.top_aligned(Self::STACK_ALIGNMENT)
    }

    fn allocate(size: u64, flags: Flags) -> PageRangeInclusive {
        assert!(size >= u64::from(Self::STACK_ALIGNMENT));

        let page_range = vmm::process_local::alloc_guarded_4k(size, flags).unwrap();

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The state of a thread.
pub enum ThreadState {
    /// The thread is running.
    Running,
    /// The thread is runnable and can be picked by a policy.
    Runnable,
    /// The thread is blocked in the waiting subsystem.
    Blocked,
    /// The thread requested termination and awaits cleanup.
    Exiting,
}
