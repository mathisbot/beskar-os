use crate::locals;
use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};
use beskar_core::arch::VirtAddr;
use core::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
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

        spawn_thread(Box::pin(clean_thread));
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
    current_thread: McsLock<Box<Thread>>,
    /// A local, atomic, priority for the current thread.
    current_priority: priority::AtomicPriority,
    should_exit_thread: AtomicBool,
    should_sleep_thread: AtomicBool,
}

impl Scheduler {
    #[must_use]
    fn new(kernel_thread: thread::Thread) -> Self {
        let current_priority = priority::AtomicPriority::new(kernel_thread.priority());

        Self {
            current_thread: McsLock::new(Box::new(kernel_thread)),
            current_priority,
            should_exit_thread: AtomicBool::new(false),
            should_sleep_thread: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn exit_current_thread(&self) {
        self.should_exit_thread.store(true, Ordering::Relaxed);
    }

    #[must_use]
    #[inline]
    pub fn current_priority(&self) -> priority::Priority {
        self.current_priority.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn change_current_thread_priority(&self, new_priority: priority::Priority) {
        self.current_priority.store(new_priority, Ordering::Relaxed);
    }

    #[must_use]
    /// Changes the internal state of the scheduler to the next thread.
    ///
    /// This function does not change the context, but will disable interrupts
    /// if scheduling was successful.
    fn reschedule(&self) -> Option<ContextSwitch> {
        self.current_thread.try_with_locked(|thread| {
            crate::arch::interrupts::int_disable();

            // Swap the current thread with the next one.
            let mut new_thread =
                Pin::into_inner(if let Some(new_thread) = QUEUE.get().unwrap().next() {
                    new_thread
                } else {
                    crate::arch::interrupts::int_enable();
                    return None;
                });

            core::mem::swap(thread.as_mut(), new_thread.as_mut());
            let mut old_thread = new_thread; // Renaming for clarity.

            // Gather information about the old thread.
            let old_priority = self
                .current_priority
                .swap(thread.priority(), Ordering::Relaxed);
            unsafe { old_thread.set_priority(old_priority) };
            let old_should_exit = self.should_exit_thread.swap(false, Ordering::Relaxed);
            let old_should_wait = self.should_sleep_thread.swap(false, Ordering::Relaxed);

            debug_assert_eq!(thread.state(), thread::ThreadState::Ready);
            unsafe { thread.set_state(thread::ThreadState::Running) };

            // Handle stack pointers.
            let old_stack = if old_should_exit {
                // In the case of the thread exiting, we cannot write to the `Thread` struct anymore.
                // Therefore, we write to a useless static variable because we won't need RSP value.
                static mut USELESS: *mut u8 = core::ptr::null_mut();
                #[expect(static_mut_refs, reason = "We do not care about data races here.")]
                // Safety: There will be data races, but we don't care lol
                unsafe {
                    &mut USELESS
                }
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

            Some(ContextSwitch {
                old_stack,
                new_stack,
                cr3,
            })
        })?
    }
}

#[must_use]
#[inline]
fn get_scheduler() -> &'static Scheduler {
    locals!().scheduler().get().unwrap()
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
    get_scheduler().reschedule()
}

#[must_use]
#[inline]
/// Returns the current thread ID.
pub fn current_thread_id() -> ThreadId {
    // Safety:
    // If the scheduler changes the values mid read,
    // it means the current thread is no longer executed.
    // Upon return, the thread will be the same as before!
    unsafe { get_scheduler().current_thread.force_lock() }.id()
}

#[must_use]
#[inline]
/// Returns the current thread's state.
pub(crate) fn current_thread_snapshot() -> thread::ThreadSnapshot {
    // Safety:
    // If the scheduler changes the values mid read,
    // it means the current thread is no longer executed.
    // Upon return, the thread will be the same as before!
    unsafe { get_scheduler().current_thread.force_lock() }.snapshot()
}

#[must_use]
#[inline]
/// Returns the current process.
pub fn current_process() -> Arc<super::Process> {
    // Safety:
    // Swapping current thread is done using a memory swap of a `Box` (pointer), so it is impossible
    // that the current thread is "partly" read before swap and "partly" after swap.
    unsafe { get_scheduler().current_thread.force_lock() }.process()
}

#[inline]
pub fn spawn_thread(thread: Pin<Box<Thread>>) {
    QUEUE.get().unwrap().append(thread);
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

#[inline]
pub fn change_current_thread_priority(priority: priority::Priority) {
    get_scheduler().change_current_thread_priority(priority);
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
    get_scheduler().exit_current_thread();

    // Try to reschedule the thread.
    thread_yield();

    // If no thread is waiting, loop.
    crate::arch::interrupts::int_enable();
    loop {
        crate::arch::halt();
    }
}

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
    get_scheduler()
        .should_sleep_thread
        .store(true, Ordering::Relaxed);
    if !thread_yield() {
        // TODO: What to do if the thread was not rescheduled?
        // Maybe push the thread in the sleeping queue and
        // halt until there is something to do?
        todo!("Thread was not rescheduled");
    }
}

/// Wakes up a thread that is sleeping.
///
/// Returns `true` if the thread was woken up, `false` otherwise.
pub fn wake_up(thread: ThreadId) -> bool {
    SLEEPING.with_locked(|wq| {
        wq.remove(&thread).is_some_and(|thread| {
            QUEUE.get().unwrap().append(Pin::new(thread));
            true
        })
    })
}
