use core::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
};

use alloc::{boxed::Box, sync::Arc};
use hyperdrive::{locks::mcs::McsLock, once::Once, queues::mpsc::MpscQueue};
use priority::ThreadQueue;
use thread::Thread;

use crate::locals;

use super::Process;

pub mod priority;
pub mod thread;

/// The time quantum for the scheduler, in milliseconds.
///
/// According to the Internet, Windows uses 20-60ms, Linux uses 0.75-6ms.
pub const SCHEDULER_QUANTUM_MS: u32 = 30;

// TODO: Runtime size for schedulers
// Currently, it takes 4KiB of memory but on a vast majority of systems, it only needs a few schedulers.
//
// Because scheduler will be playing with context switching, we cannot acquire locks.
// Therefore, we will have to use unsafe mutable statics, in combination with `AtomicBool`s.
static SCHEDULERS: [Once<Scheduler>; 256] = [const { Once::uninit() }; 256];

// It is backed by a Multiple Producer Single Consumer queue.
// It would be a better choice to use a Multiple Producer Multiple Consumer queue,
// but the only implemention I know uses a fixed size buffer and I don't want to bound the number of threads.
/// A queue for threads.
static QUEUE: Once<priority::RoundRobinQueues> = Once::uninit();

/// A queue for finished threads.
static FINISHED_QUEUE: Once<MpscQueue<Thread>> = Once::uninit();

/// This function initializes the scheduler with the kernel thread.
///
/// ## Safety
///
/// This function should only be called once, and only by the kernel, with the kernel thread.
pub unsafe fn init(kernel_thread: thread::Thread) {
    static SPAWN_CLEAN_THREAD: Once<()> = Once::uninit();

    let kernel_process = kernel_thread.process();

    QUEUE.call_once(|| priority::RoundRobinQueues::create(kernel_process.clone()));
    FINISHED_QUEUE.call_once(|| MpscQueue::new(Box::pin(Thread::new_stub(kernel_process.clone()))));

    let scheduler = Scheduler::new(kernel_thread);
    SCHEDULERS[locals!().core_id()].call_once(|| scheduler);

    SPAWN_CLEAN_THREAD.call_once(|| {
        let clean_thread = Thread::new(
            kernel_process,
            priority::Priority::Low,
            alloc::vec![0; 1024 * 128],
            clean_thread,
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
}

impl Scheduler {
    #[must_use]
    fn new(kernel_thread: thread::Thread) -> Self {
        let current_priority = priority::AtomicPriority::new(kernel_thread.priority());

        Self {
            current_thread: McsLock::new(Box::new(kernel_thread)),
            current_priority,
            should_exit_thread: AtomicBool::new(false),
        }
    }

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

            // Handle stack pointers.
            let old_stack = old_thread.last_stack_ptr_mut();
            let new_stack = thread.last_stack_ptr();

            if old_should_exit {
                // As the scheduler must not acquire locks, it cannot drop heap-allocated memory.
                // This job should be done by a cleaning thread.
                FINISHED_QUEUE.get().unwrap().enqueue(Pin::new(old_thread));
            } else {
                QUEUE.get().unwrap().append(Pin::new(old_thread));
            }

            let cr3 = thread.process().address_space().cr3_raw();

            Some(ContextSwitch {
                old_stack,
                new_stack,
                cr3,
            })
        })?
    }
}

#[inline]
fn get_scheduler() -> &'static Scheduler {
    SCHEDULERS[locals!().core_id()].get().unwrap()
}

extern "C" fn clean_thread() {
    // FIXME: If the cleaning process starts very soon,
    // it often results in a bad free.
    // idk what happens, so let's wait for a bit.
    crate::time::wait(crate::time::Duration::from_millis(250));
    loop {
        if let Some(thread) = FINISHED_QUEUE.get().unwrap().dequeue() {
            drop(thread);
        } else {
            core::hint::spin_loop();
            thread_yield();
        }
    }
}

#[must_use]
#[inline]
/// Reschedules the scheduler.
///
/// If rescheduling happens, interrupts are disabled.
///
/// ## Warning
///
/// This function does not perform the context switch.
pub(crate) fn reschedule() -> Option<ContextSwitch> {
    get_scheduler().reschedule()
}

#[must_use]
/// Returns the current thread ID.
pub fn current_thread_id() -> thread::ThreadId {
    // Safety:
    // Swapping current thread is done using a memory swap of a `Box` (pointer), so it is impossible
    // that the current thread is "partly" read before swap and "partly" after swap.
    unsafe { get_scheduler().current_thread.force_lock() }.id()
}

#[must_use]
/// Returns the current process.
pub fn current_process() -> Arc<Process> {
    // Safety:
    // Swapping current thread is done using a memory swap of a `Box` (pointer), so it is impossible
    // that the current thread is "partly" read before swap and "partly" after swap.
    unsafe { get_scheduler().current_thread.force_lock() }.process()
}

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

pub fn change_current_thread_priority(priority: priority::Priority) {
    get_scheduler().change_current_thread_priority(priority);
}

/// Exits the current thread.
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

pub fn thread_yield() {
    let context_switch = reschedule();

    // If no other thread is waiting, then we can't continue doing nothing
    // in the current thread.
    if let Some(context_switch) = context_switch {
        unsafe { context_switch.perform() };
    }
}

pub fn is_scheduling_init() -> bool {
    SCHEDULERS[locals!().core_id()].get().is_some()
}
