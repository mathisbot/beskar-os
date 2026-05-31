#![allow(
    clippy::vec_box,
    reason = "Boxed threads are necessary for dynamic allocation"
)]

use crate::locals;
use ::wait::{WakeCause, WakeResult};
use alloc::{boxed::Box, sync::Arc};
use beskar_core::{arch::VirtAddr, process::SleepHandle};
use beskar_hal::{instructions::without_interrupts, paging::page_table::Flags};
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use hyperdrive::{call_once, locks::mcs::McsLock, once::Once, queues::mpsc::MpscQueue};
use priority::ThreadQueue;
use thread::{Thread, ThreadId};

mod priority;
pub use priority::Priority;
pub mod thread;
mod wait;

static SCHEDULER_SWITCH: AtomicBool = AtomicBool::new(false);

/// The time quantum for the scheduler, in milliseconds.
///
/// According to the Internet, Windows uses 20-60ms, Linux uses 0.75-6ms.
pub const SCHEDULER_QUANTUM_MS: u32 = crate::arch::apic::MS_PER_INTERRUPT;

const IDLE_THREADS_PER_CORE: usize = 2;

// It is backed by a Multiple Producer Single Consumer queue.
// It would be a better choice to use a Multiple Producer Multiple Consumer queue,
// but the only implemention I know uses a fixed size buffer and I don't want to bound the number of threads.
/// A queue for threads.
static QUEUE: Once<priority::RoundRobinQueues> = Once::uninit();

/// A queue for finished threads.
static FINISHED: Once<MpscQueue<Thread>> = Once::uninit();

/// This function initializes the scheduler with the kernel thread.
///
/// # Safety
///
/// This function should only be called once, and only by the kernel, with the kernel thread.
pub unsafe fn init(kernel_thread: thread::Thread) {
    let kernel_process = kernel_thread.process();

    QUEUE.call_once(|| priority::RoundRobinQueues::new(kernel_process.clone()));
    FINISHED.call_once(|| MpscQueue::new(Box::new(Thread::new_stub(kernel_process.clone()))));
    call_once!(wait::init());

    let scheduler = Scheduler::new(kernel_thread);
    locals!().scheduler().call_once(|| scheduler);

    for _ in 0..IDLE_THREADS_PER_CORE {
        Thread::builder(kernel_process.clone(), idle)
            .priority(Priority::Idle)
            .stack_pool(4096, Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE)
            .spawn();
    }

    call_once!({
        Thread::builder(kernel_process, guard_thread)
            .priority(priority::Priority::Low)
            .stack_pool(
                1024 * 32,
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            )
            .spawn();
    });
}

#[must_use]
#[inline]
pub fn scheduler_tick() -> Option<ContextSwitch> {
    wake_blocked_threads();

    // Attempt to reschedule
    crate::process::scheduler::reschedule(RescheduleReason::QuantumExpired)
}

#[derive(Debug, Clone, Copy)]
pub struct ContextSwitch {
    old_stack: *mut *mut u8,
    new_stack: *const u8,
    cr3: u64,
}

impl ContextSwitch {
    #[inline]
    /// Performs the context switch.
    ///
    /// # Safety
    ///
    /// See `kernel::arch::context::context_switch`.
    pub unsafe fn perform(&self) {
        unsafe { crate::arch::context::switch(self.old_stack, self.new_stack, self.cr3) };
    }
}

pub struct Scheduler {
    current: McsLock<Box<Thread>>,
    should_exit: AtomicBool,
}

impl Scheduler {
    #[must_use]
    #[inline]
    fn new(kernel_thread: thread::Thread) -> Self {
        Self {
            current: McsLock::new(Box::new(kernel_thread)),
            should_exit: AtomicBool::new(false),
        }
    }

    #[inline]
    /// Sets an inner flag to indicate that the current thread should exit.
    ///
    /// This function does not perform the context switch, but it will
    /// ensure that the next time the scheduler is called, the current thread
    /// will be exited.
    fn set_exit(&self) {
        self.should_exit.store(true, Ordering::Relaxed);
    }

    #[inline]
    /// Arms a previously registered wait token on the current thread.
    fn arm_wait_token(&self, token: u64) {
        self.current.with_locked(|thread| {
            debug_assert_eq!(
                thread.wait_token(),
                0,
                "thread has pending wait token; finalize_block not called"
            );
            thread.arm_wait(token);
        });
    }

    #[must_use]
    /// Changes the internal state of the scheduler to the next thread.
    ///
    /// This function does not change the context, but will disable interrupts
    /// if scheduling was successful.
    fn reschedule(&self, reason: RescheduleReason) -> Option<ContextSwitch> {
        self.current
            .try_with_locked(|thread| {
                // FIXME: cpu_time_ms is charged on all reschedule calls including explicit yields.
                // This over-counts time spent on ExplicitYield.
                thread.stats_mut().cpu_time_ms += u64::from(SCHEDULER_QUANTUM_MS);

                let queue = QUEUE.get()?;
                let Some(mut candidate) = queue.pop_best() else {
                    debug_assert!(thread.priority() == Priority::Idle);
                    return None;
                };

                let action = self.next_action(thread);

                let should_stay = matches!(action, ThreadAction::Runnable)
                    && !queue.should_switch(thread, &candidate, reason);
                if should_stay {
                    queue.append(candidate);
                    return None;
                }

                // Swap the current thread with the candidate from the ready queues.
                core::mem::swap(thread.as_mut(), candidate.as_mut());
                let mut old_thread = candidate; // Renaming for clarity.

                debug_assert_eq!(thread.state(), thread::ThreadState::Runnable);
                unsafe { thread.set_state(thread::ThreadState::Running) };

                // Handle stack pointers.
                let old_stack = Self::old_stack_pointer(&action, &mut old_thread);
                let new_stack = thread.last_stack_ptr();

                if let Some(rsp0) = thread.snapshot().kernel_stack_top() {
                    let rsp0 = rsp0.as_ptr();
                    if let Some(tss) = unsafe { locals!().gdt().force_lock() }.tss_mut() {
                        tss.privilege_stack_table[0] = VirtAddr::from_ptr(rsp0);
                    } else {
                        beskar_core::debug_panic!("TSS not found when setting syscall stack");
                    }
                    locals!().set_syscall_stack(rsp0);
                }

                let cr3 = thread.process().address_space().cr3_raw();

                crate::arch::fpu::on_thread_switch(&mut old_thread);

                Self::stage_old_thread(action, old_thread);

                beskar_hal::instructions::int_disable();

                Some(ContextSwitch {
                    old_stack,
                    new_stack,
                    cr3,
                })
            })
            .flatten()
    }

    #[inline]
    fn next_action(&self, current: &Thread) -> ThreadAction {
        if self.should_exit.swap(false, Ordering::Relaxed) {
            return ThreadAction::Exit;
        }

        if current.wait_token() != 0 {
            ThreadAction::Block
        } else {
            ThreadAction::Runnable
        }
    }

    #[inline]
    fn old_stack_pointer(action: &ThreadAction, old_thread: &mut Thread) -> *mut *mut u8 {
        match action {
            ThreadAction::Exit => {
                // In the case of the thread exiting, we cannot write to the `Thread` struct anymore.
                // Therefore, we write to a useless static variable because we won't need RSP value.
                static USELESS: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
                // Note: there may be data races here, but we do not care.
                USELESS.as_ptr()
            }
            _ => {
                // Safety: context switching uses a `mov` instruction to write to the old stack pointer,
                // which is atomic by nature.
                unsafe { old_thread.last_stack_ptr_mut() }
            }
        }
    }

    fn stage_old_thread(action: ThreadAction, mut old_thread: Box<Thread>) {
        match action {
            ThreadAction::Exit => {
                unsafe { old_thread.set_state(thread::ThreadState::Exiting) };
                // As the scheduler must not acquire locks, it cannot drop heap-allocated memory.
                // This job should be done by a cleaning thread.
                FINISHED.get().unwrap().enqueue(old_thread);
            }
            ThreadAction::Block => match wait::commit_block(old_thread) {
                ::wait::BlockCommit::Parked => {}
                ::wait::BlockCommit::Requeue(mut thread) => {
                    unsafe { thread.set_state(thread::ThreadState::Runnable) };
                    QUEUE.get().unwrap().append(thread);
                }
            },
            ThreadAction::Runnable => {
                unsafe { old_thread.set_state(thread::ThreadState::Runnable) };
                QUEUE.get().unwrap().append(old_thread);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ThreadAction {
    Exit,
    Block,
    Runnable,
}

#[derive(Debug, Clone, Copy)]
enum RescheduleReason {
    /// Periodic timer tick (time slice expired).
    QuantumExpired,
    /// Explicit yield request from the running thread.
    ExplicitYield,
}

impl RescheduleReason {
    #[inline]
    const fn should_rotate(self) -> bool {
        matches!(self, Self::QuantumExpired | Self::ExplicitYield)
    }
}

#[inline]
/// Executes a closure with the scheduler.
///
/// Note that this function does not involve any locking,
/// it simply makes sure that interrupts are disabled.
fn with_scheduler<R, F: FnOnce(&'static Scheduler) -> R>(f: F) -> R {
    without_interrupts(|| {
        let scheduler = locals!().scheduler().get().unwrap();
        f(scheduler)
    })
}

fn wake_blocked_threads() {
    let now = crate::time::now();
    for thread in wait::collect_timed_out(now) {
        enqueue_ready_thread(thread);
    }
}

fn enqueue_ready_thread(mut thread: Box<Thread>) {
    unsafe { thread.set_state(thread::ThreadState::Runnable) };
    QUEUE.get().unwrap().append(thread);
}

/// A thread should be spawned with this function.
///
/// This function endlessly loops and performs the following tasks:
/// - Drops finished threads.
/// - Yields the CPU if no thread is ready to run.
extern "C" fn guard_thread() -> ! {
    loop {
        while let Some(thread) = FINISHED.get().unwrap().dequeue() {
            debug_assert!(thread.state() == thread::ThreadState::Exiting);
            drop(thread);
        }
        thread_yield();
    }
}

extern "C" fn idle() -> ! {
    loop {
        crate::arch::halt();
    }
}

#[must_use]
#[inline]
/// Reschedules the scheduler.
///
/// If rescheduling happens (i.e. returned value is `Some`), interrupts are disabled.
///
/// # Warning
///
/// This function does not perform the context switch.
fn reschedule(reason: RescheduleReason) -> Option<ContextSwitch> {
    if SCHEDULER_SWITCH.load(Ordering::Acquire) {
        with_scheduler(|scheduler| scheduler.reschedule(reason))
    } else {
        None
    }
}

#[must_use]
#[inline]
/// Returns the current thread ID.
pub fn current_thread_id() -> ThreadId {
    with_scheduler(|scheduler| {
        // Safety:
        // Interrupts are disabled, so the current thread cannot change.
        unsafe { scheduler.current.force_lock() }.id()
    })
}

#[must_use]
#[inline]
/// Returns the current thread's state.
pub(crate) fn current_thread_snapshot() -> thread::ThreadSnapshot {
    with_scheduler(|scheduler| {
        // Safety:
        // Interrupts are disabled, so the current thread cannot change.
        unsafe { scheduler.current.force_lock() }.snapshot()
    })
}

#[must_use]
#[inline]
/// Returns the current process.
pub fn current_process() -> Arc<super::Process> {
    with_scheduler(|scheduler| {
        // Safety:
        // Interrupts are disabled, so the current thread cannot change.
        unsafe { scheduler.current.force_lock() }.process()
    })
}

#[inline]
pub fn spawn_thread(thread: Box<Thread>) {
    enqueue_ready_thread(thread);
}

/// Sets the scheduling of the scheduler.
pub fn set_scheduling(enable: bool) {
    SCHEDULER_SWITCH.store(enable, Ordering::Release);
}

/// Exits the current thread.
///
/// This function will enable interrupts, otherwise the system would halt.
///
/// # Safety
///
/// The context will be brutally switched without returning.
/// If any locks are acquired, they will be poisoned.
pub unsafe fn exit_current_thread() -> ! {
    with_scheduler(Scheduler::set_exit);

    // Try to reschedule the thread.
    thread_yield();

    // If no thread is waiting, loop.
    beskar_hal::instructions::int_enable();
    loop {
        crate::arch::halt();
    }
}

/// Hint to the scheduler to reschedule the current thread.
pub fn thread_yield() {
    let context_switch = reschedule(RescheduleReason::ExplicitYield);

    if let Some(cs) = context_switch {
        unsafe { cs.perform() };
    }
}

#[must_use]
#[inline]
pub fn is_scheduling_init() -> bool {
    locals!().scheduler().is_initialized()
}

/// A back-off stategy that yields the CPU.
pub struct Yield;

impl hyperdrive::locks::RelaxStrategy for Yield {
    #[inline]
    fn relax() {
        thread_yield();
    }
}

#[must_use]
/// Generic blocking primitive used by synchronization objects.
///
/// Returns the wake outcome once the current thread becomes runnable again.
pub fn wait(wait: ::wait::WaitRequest) -> ::wait::WakeResult {
    let _ = arm_wait(wait);
    thread_yield();
    wait_completion()
}

#[must_use]
/// Registers the current thread in the wait database and arms its wait token.
fn arm_wait(wait: ::wait::WaitRequest) -> ThreadId {
    let tid = current_thread_id();
    let token = wait::register_wait(tid, wait);

    with_scheduler(|scheduler| scheduler.arm_wait_token(token));
    tid
}

#[must_use]
/// Wait while the supplied predicate says the caller should still block.
pub fn wait_if<F>(wait: ::wait::WaitRequest, should_block: F) -> ::wait::WakeResult
where
    F: FnOnce() -> bool,
{
    let tid = arm_wait(wait);

    if !should_block() {
        let _ = wait::wake_thread(tid, WakeResult::cancelled());
    }

    thread_yield();
    wait_completion()
}

#[must_use]
fn wait_completion() -> ::wait::WakeResult {
    loop {
        let (wait_token, wake_result) = with_scheduler(|scheduler| {
            // Safety:
            // Interrupts are disabled, so the current thread cannot change.
            let thread = unsafe { scheduler.current.force_lock() };
            (thread.wait_token(), thread.wake_result())
        });

        // The wait operation is fully completed once the token is disarmed.
        if wait_token == 0 {
            return wake_result;
        }

        // Blocking commit was deferred due to lock contention; retry by yielding again.
        thread_yield();
    }
}

/// Signal an event handle and wake a single sleeper waiting on it.
#[expect(clippy::must_use_candidate)]
pub fn wake_event_single(handle: SleepHandle) -> bool {
    let wake = wait::wake_event_single(handle);
    match wake {
        ::wait::WakeHit::None => false,
        ::wait::WakeHit::WokeParking => true,
        ::wait::WakeHit::Ready(thread) => {
            enqueue_ready_thread(thread);
            true
        }
    }
}

/// Signal an event handle and wake all sleepers waiting on it.
#[expect(clippy::must_use_candidate)]
pub fn wake_event_all(handle: SleepHandle) -> usize {
    let ready = wait::wake_event_all(handle);
    let count = ready.len();
    for thread in ready {
        enqueue_ready_thread(thread);
    }
    count
}

/// Wakes up a thread that is sleeping.
///
/// Returns `true` if the thread was woken up,
/// `false` if the thread was not sleeping.
#[expect(clippy::must_use_candidate)]
pub fn wake_up(thread: ThreadId) -> bool {
    match wait::wake_thread(thread, WakeResult::new(WakeCause::Killed, 0)) {
        ::wait::WakeHit::None => false,
        ::wait::WakeHit::WokeParking => true,
        ::wait::WakeHit::Ready(thread) => {
            enqueue_ready_thread(thread);
            true
        }
    }
}
