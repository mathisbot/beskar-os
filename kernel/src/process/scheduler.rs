use crate::locals;
use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};
use beskar_core::arch::VirtAddr;
use beskar_hal::instructions::without_interrupts;
use core::{
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};
use hyperdrive::{locks::mcs::McsLock, once::Once, queues::mpsc::MpscQueue};
use priority::ThreadQueue;
use thread::{Thread, ThreadId};

pub mod priority;
pub mod thread;

/// The time quantum for the scheduler, in milliseconds.
///
/// According to the Internet, Windows uses 20-60ms, Linux uses 0.75-6ms.
pub const SCHEDULER_QUANTUM_MS: u32 = 30;

// It is backed by a Multiple Producer Single Consumer queue.
// It would be a better choice to use a Multiple Producer Multiple Consumer queue,
// but the only implemention I know uses a fixed size buffer and I don't want to bound the number of threads.
/// A queue for threads.
static QUEUE: Once<priority::RoundRobinQueues> = Once::uninit();

/// A queue for finished threads.
static FINISHED: Once<MpscQueue<Thread>> = Once::uninit();

static SLEEPING: McsLock<BTreeMap<ThreadId, Box<Thread>>> = McsLock::new(BTreeMap::new());

/// This function initializes the scheduler with the kernel thread.
///
/// ## Safety
///
/// This function should only be called once, and only by the kernel, with the kernel thread.
pub unsafe fn init(kernel_thread: thread::Thread) {
    static SPAWN_GUARD_THREAD: Once<()> = Once::uninit();

    let kernel_process = kernel_thread.process();

    QUEUE.call_once(|| priority::RoundRobinQueues::new(kernel_process.clone()));
    FINISHED.call_once(|| MpscQueue::new(Box::pin(Thread::new_stub(kernel_process.clone()))));

    let scheduler = Scheduler::new(kernel_thread);
    locals!().scheduler().call_once(|| scheduler);

    SPAWN_GUARD_THREAD.call_once(|| {
        let clean_thread = Thread::new(
            kernel_process,
            priority::Priority::Low,
            alloc::vec![0; 1024 * 128],
            guard_thread,
        );

        spawn_thread(Box::new(clean_thread));
    });
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
    /// ## Safety
    ///
    /// See `kernel::arch::context::context_switch`.
    pub unsafe fn perform(&self) {
        unsafe { crate::arch::context::switch(self.old_stack, self.new_stack, self.cr3) };
    }
}

pub struct Scheduler {
    current: McsLock<Box<Thread>>,
    /// A local, atomic, priority for the current thread.
    should_exit: AtomicBool,
    should_sleep: AtomicBool,
}

impl Scheduler {
    #[must_use]
    #[inline]
    fn new(kernel_thread: thread::Thread) -> Self {
        Self {
            current: McsLock::new(Box::new(kernel_thread)),
            should_exit: AtomicBool::new(false),
            should_sleep: AtomicBool::new(false),
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
    fn set_sleep(&self) {
        self.should_sleep.store(true, Ordering::Relaxed);
    }

    #[must_use]
    /// Changes the internal state of the scheduler to the next thread.
    ///
    /// This function does not change the context, but will disable interrupts
    /// if scheduling was successful.
    fn reschedule(&self) -> Option<ContextSwitch> {
        self.current
            .try_with_locked(|thread| {
                // Swap the current thread with the next one.
                let mut new_thread = Pin::into_inner(QUEUE.get().unwrap().next()?);

                core::mem::swap(thread.as_mut(), new_thread.as_mut());
                let mut old_thread = new_thread; // Renaming for clarity.

                // Gather information about the old thread.
                let old_should_exit = self.should_exit.swap(false, Ordering::Relaxed);
                let old_should_wait = self.should_sleep.swap(false, Ordering::Relaxed);

                debug_assert_eq!(thread.state(), thread::ThreadState::Ready);
                unsafe { thread.set_state(thread::ThreadState::Running) };

                // Handle stack pointers.
                let old_stack = if old_should_exit {
                    // In the case of the thread exiting, we cannot write to the `Thread` struct anymore.
                    // Therefore, we write to a useless static variable because we won't need RSP value.
                    static USELESS: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
                    // Note: there may be data races here, but we do not care.
                    USELESS.as_ptr()
                } else {
                    old_thread.last_stack_ptr_mut()
                };
                let new_stack = thread.last_stack_ptr();

                let cr3 = thread.process().address_space().cr3_raw();
                if let Some(tls) = thread.tls() {
                    crate::arch::locals::store_thread_locals(tls);
                }

                if let Some(rsp0) = thread.snapshot().kernel_stack_top() {
                    let tss = unsafe { locals!().gdt().force_lock() }.tss_mut().unwrap();
                    tss.privilege_stack_table[0] = VirtAddr::from_ptr(rsp0.as_ptr());
                }

                if old_should_exit {
                    // As the scheduler must not acquire locks, it cannot drop heap-allocated memory.
                    // This job should be done by a cleaning thread.
                    FINISHED.get().unwrap().enqueue(Pin::new(old_thread));
                } else if old_should_wait {
                    unsafe { old_thread.set_state(thread::ThreadState::Sleeping) };
                    SLEEPING.with_locked(|wq| wq.insert(old_thread.id(), old_thread));
                } else {
                    unsafe { old_thread.set_state(thread::ThreadState::Ready) };
                    QUEUE.get().unwrap().append(Pin::new(old_thread));
                }

                beskar_hal::instructions::int_disable();

                Some(ContextSwitch {
                    old_stack,
                    new_stack,
                    cr3,
                })
            })
            .flatten()
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
        if !thread_yield() {
            crate::arch::halt();
        }
    }
}

#[must_use]
#[inline]
/// Reschedules the scheduler.
///
/// If rescheduling happens (i.e. returned value is `Some`), interrupts are disabled.
///
/// ## Warning
///
/// This function does not perform the context switch.
pub(crate) fn reschedule() -> Option<ContextSwitch> {
    with_scheduler(Scheduler::reschedule)
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
pub fn spawn_thread(mut thread: Box<Thread>) {
    unsafe { thread.set_state(thread::ThreadState::Ready) };
    QUEUE.get().unwrap().append(Pin::new(thread));
}

/// Sets the scheduling of the scheduler.
///
/// What this function really does is enabling the timer interrupt.
pub fn set_scheduling(enable: bool) {
    use crate::arch::apic::timer;

    locals!().lapic().with_locked_if_init(|lapic| {
        const TIMER_DIVIDER: timer::Divider = timer::Divider::Eight;

        let timer = lapic.timer();

        let ticks_per_ms = timer.rate_mhz().unwrap().get() * 1_000 / TIMER_DIVIDER.as_u32();
        let ticks = SCHEDULER_QUANTUM_MS * ticks_per_ms;

        lapic.timer().set(if enable {
            timer::Mode::Periodic(timer::ModeConfiguration::new(TIMER_DIVIDER, ticks))
        } else {
            timer::Mode::Inactive
        });
    });
}

/// Exits the current thread.
///
/// This function will enable interrupts, otherwise the system would halt.
///
/// ## Safety
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

#[expect(clippy::must_use_candidate, reason = "Yields the CPU")]
/// Hint to the scheduler to reschedule the current thread.
///
/// Returns `true` if the thread was rescheduled, `false` otherwise.
pub fn thread_yield() -> bool {
    let context_switch = reschedule();

    context_switch.is_some_and(|cs| {
        unsafe { cs.perform() };
        true
    })
}

#[must_use]
#[inline]
pub fn is_scheduling_init() -> bool {
    locals!().scheduler().is_initialized()
}

/// A back-off stategy that yields the CPU.
pub struct Yield;

impl hyperdrive::locks::BackOff for Yield {
    #[inline]
    fn back_off() {
        thread_yield();
    }
}

/// Put the current thread to sleep.
pub fn sleep() {
    with_scheduler(|scheduler| {
        scheduler.set_sleep();
    });

    if !thread_yield() {
        // TODO: What to do if the thread was not rescheduled?
        // Maybe push the thread in the sleeping queue and
        // halt until there is something to do?
        todo!("Thread was not rescheduled");
    }
}

/// Wakes up a thread that is sleeping.
///
/// Returns `true` if the thread was woken up,
/// `false` if the thread was not sleeping.
pub fn wake_up(thread: ThreadId) -> bool {
    SLEEPING.with_locked(|wq| {
        wq.remove(&thread).is_some_and(|thread| {
            spawn_thread(thread);
            true
        })
    })
}
