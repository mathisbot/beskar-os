use core::sync::atomic::{AtomicBool, Ordering};

use alloc::sync::Arc;
use thread::Thread;

use super::Process;

pub mod priority;
pub mod thread;

// Because scheduler will be playing with context switching, we cannot acquire locks.
// Therefore, we will have to use unsafe mutable statics, in combination with `AtomicBool`s.
static mut SCHEDULER: Option<Scheduler<priority::RoundRobinQueues>> = None;

/// This function initializes the scheduler with the kernel thread.
///
/// ## Safety
///
/// This function should only be called once, and only by the kernel, with the kernel thread.
pub unsafe fn init(kernel_thread: thread::Thread) {
    let scheduler = Scheduler::new(kernel_thread);
    // Safety:
    // Function safety guards.
    unsafe {
        SCHEDULER = Some(scheduler);
    }
}

pub struct Scheduler<Q: priority::ThreadQueue> {
    current_thread: Thread,
    /// A local, atomic, priority for the current thread.
    current_priority: priority::AtomicPriority,
    should_exit_thread: AtomicBool,
    queues: Q,
}

impl<Q: priority::ThreadQueue> Scheduler<Q> {
    #[must_use]
    fn new(kernel_thread: thread::Thread) -> Self
    where
        Q: Default,
    {
        let current_priority = priority::AtomicPriority::new(kernel_thread.priority());

        Self {
            current_thread: kernel_thread,
            current_priority,
            should_exit_thread: AtomicBool::new(false),
            queues: Q::default(),
        }
    }

    pub fn exit_current_thread(&self) {
        self.should_exit_thread.store(true, Ordering::Relaxed);
    }

    pub fn schedule_thread(&mut self, thread: thread::Thread) {
        self.queues.append(thread);
    }

    #[must_use]
    #[inline]
    pub const fn current_thread(&self) -> &Thread {
        &self.current_thread
    }

    #[must_use]
    #[inline]
    pub fn current_process(&self) -> Arc<Process> {
        self.current_thread.process()
    }

    #[must_use]
    #[inline]
    pub const fn current_priority(&self) -> &priority::AtomicPriority {
        &self.current_priority
    }

    /// Changes the internal state of the scheduler to the next thread.
    ///
    /// This function does not change the context or else.
    pub fn reschedule(&mut self) {
        static IN_RESCHEDULE: AtomicBool = AtomicBool::new(false);

        // We cannot acquire locks, so we imitate one with an `AtomicBool`.
        if IN_RESCHEDULE
            .compare_exchange(false, false, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        x86_64::instructions::interrupts::disable();

        if !self.should_exit_thread.load(Ordering::Acquire) {
            self.queues.append(self.current_thread().clone());
        }
        self.should_exit_thread.store(false, Ordering::Relaxed);
        self.current_thread = self.queues.next();

        // Safety:
        // Interrupts are indeed disabled at the start of the function.
        // unsafe {
        //     crate::cpu::context::context_switch(old_stack, new_stack, cr3);
        // }
    }
}
