#![allow(
    clippy::vec_box,
    reason = "Boxed threads are necessary for dynamic allocation"
)]

use crate::{locals, time::Duration};
use alloc::{boxed::Box, sync::Arc};
use beskar_core::{
    arch::VirtAddr,
    process::{AtomicSleepReason, SleepHandle, SleepReason},
    time::Instant,
};
use beskar_hal::instructions::without_interrupts;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use hyperdrive::{call_once, locks::mcs::McsLock, once::Once, queues::mpsc::MpscQueue};
use priority::ThreadQueue;
use thread::{Thread, ThreadId};

mod priority;
pub use priority::Priority;
mod sleep;
use sleep::SleepQueues;
pub mod thread;

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

/// Sleep queues for timed and event-based sleepers.
static SLEEPING: McsLock<SleepQueues> = McsLock::new(SleepQueues::new());

/// This function initializes the scheduler with the kernel thread.
///
/// # Safety
///
/// This function should only be called once, and only by the kernel, with the kernel thread.
pub unsafe fn init(kernel_thread: thread::Thread) {
    let kernel_process = kernel_thread.process();

    QUEUE.call_once(|| priority::RoundRobinQueues::new(kernel_process.clone()));
    FINISHED.call_once(|| MpscQueue::new(Box::new(Thread::new_stub(kernel_process.clone()))));

    let scheduler = Scheduler::new(kernel_thread);
    locals!().scheduler().call_once(|| scheduler);

    for _ in 0..IDLE_THREADS_PER_CORE {
        let local_idle_thread = Thread::new(
            kernel_process.clone(),
            Priority::Low,
            alloc::vec![0; 8 * 1024],
            idle,
        );
        spawn_thread(Box::new(local_idle_thread));
    }

    call_once!({
        let clean_thread = Thread::new(
            kernel_process,
            priority::Priority::Low,
            alloc::vec![0; 1024 * 128],
            guard_thread,
        );

        spawn_thread(Box::new(clean_thread));
    });
}

#[must_use]
#[inline]
pub fn scheduler_tick() -> Option<ContextSwitch> {
    wake_sleeping_threads();

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
    sleep_intent: AtomicSleepReason,
}

impl Scheduler {
    #[must_use]
    #[inline]
    fn new(kernel_thread: thread::Thread) -> Self {
        Self {
            current: McsLock::new(Box::new(kernel_thread)),
            should_exit: AtomicBool::new(false),
            sleep_intent: AtomicSleepReason::new(None),
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
    /// Sets an inner flag to indicate that the current thread should sleep.
    ///
    /// This function does not perform the context switch, but it will
    /// ensure that the next time the scheduler is called, the current thread
    /// will be put to sleep.
    fn set_sleep(&self, reason: SleepReason) {
        self.sleep_intent.store(Some(reason), Ordering::Release);
    }

    #[must_use]
    /// Changes the internal state of the scheduler to the next thread.
    ///
    /// This function does not change the context, but will disable interrupts
    /// if scheduling was successful.
    fn reschedule(&self, reason: RescheduleReason) -> Option<ContextSwitch> {
        self.current
            .try_with_locked(|thread| {
                thread.stats_mut().cpu_time_ms += u64::from(SCHEDULER_QUANTUM_MS);

                let queue = QUEUE.get()?;
                let Some(mut candidate) = queue.pop_best() else {
                    // No runnable threads available. This can happen when all idle threads
                    // are already running on other cores. Keep the current thread running.
                    debug_assert!(thread.priority() == Priority::Idle);
                    return None;
                };

                let action = self.next_action();

                let should_stay = matches!(action, ThreadAction::Ready)
                    && !queue.should_switch(thread, &candidate, reason);
                if should_stay {
                    queue.append(candidate);
                    return None;
                }

                // Swap the current thread with the candidate from the ready queues.
                core::mem::swap(thread.as_mut(), candidate.as_mut());
                let mut old_thread = candidate; // Renaming for clarity.

                debug_assert_eq!(thread.state(), thread::ThreadState::Ready);
                unsafe { thread.set_state(thread::ThreadState::Running) };

                // Handle stack pointers.
                let old_stack = Self::old_stack_pointer(&action, &mut old_thread);
                let new_stack = thread.last_stack_ptr();

                let cr3 = thread.process().address_space().cr3_raw();
                if let Some(tls) = thread.tls() {
                    crate::arch::locals::store_thread_locals(tls);
                }

                if let Some(rsp0) = thread.snapshot().kernel_stack_top() {
                    let tss = unsafe { locals!().gdt().force_lock() }.tss_mut().unwrap();
                    tss.privilege_stack_table[0] = VirtAddr::from_ptr(rsp0.as_ptr());
                }

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
    fn next_action(&self) -> ThreadAction {
        if self.should_exit.swap(false, Ordering::Relaxed) {
            ThreadAction::Exit
        } else if let Some(reason) = self.sleep_intent.swap(None, Ordering::Acquire) {
            ThreadAction::Sleep(reason)
        } else {
            ThreadAction::Ready
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
                // As the scheduler must not acquire locks, it cannot drop heap-allocated memory.
                // This job should be done by a cleaning thread.
                FINISHED.get().unwrap().enqueue(old_thread);
            }
            ThreadAction::Sleep(reason) => {
                unsafe { old_thread.set_state(thread::ThreadState::Sleeping) };
                SLEEPING.with_locked(|queues| queues.insert(reason, old_thread));
            }
            ThreadAction::Ready => {
                unsafe { old_thread.set_state(thread::ThreadState::Ready) };
                QUEUE.get().unwrap().append(old_thread);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ThreadAction {
    Exit,
    Sleep(SleepReason),
    Ready,
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

fn wake_sleeping_threads() {
    let now = crate::time::now();
    SLEEPING.try_with_locked(|sleepers| {
        while let Some(thread) = sleepers.pop_ready(now) {
            enqueue_ready_thread(thread);
        }
    });
}

fn enqueue_ready_thread(mut thread: Box<Thread>) {
    unsafe { thread.set_state(thread::ThreadState::Ready) };
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

/// Put the current thread to sleep.
pub fn sleep() {
    request_sleep(SleepReason::Indefinite);
}

/// Sleep for a relative duration.
pub fn sleep_for(duration: Duration) {
    request_sleep(SleepReason::for_duration(crate::time::now(), duration));
}

/// Sleep until an absolute deadline.
pub fn sleep_until(deadline: Instant) {
    request_sleep(SleepReason::Until(deadline));
}

/// Sleep until the given handle is signalled by another subsystem.
pub fn sleep_on(handle: SleepHandle) {
    request_sleep(SleepReason::Event(handle));
}

fn request_sleep(reason: SleepReason) {
    with_scheduler(|scheduler| scheduler.set_sleep(reason));
    thread_yield();
}

/// Signal an event handle and wake a single sleeper waiting on it.
pub fn wake_event_single(handle: SleepHandle) -> bool {
    let ready = SLEEPING.with_locked(|sleepers| sleepers.wake_event_single(handle));
    ready.is_some_and(|thread| {
        enqueue_ready_thread(thread);
        true
    })
}

/// Signal an event handle and wake all sleepers waiting on it.
pub fn wake_event_all(handle: SleepHandle) -> usize {
    let ready = SLEEPING.with_locked(|sleepers| sleepers.wake_event_all(handle));
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
pub fn wake_up(thread: ThreadId) -> bool {
    SLEEPING
        .with_locked(|sleepers| sleepers.wake_thread(thread))
        .is_some_and(|thread| {
            enqueue_ready_thread(thread);
            true
        })
}
