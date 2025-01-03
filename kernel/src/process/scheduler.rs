use core::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
};

use alloc::{boxed::Box, sync::Arc};
use thread::Thread;

use crate::locals;

use super::Process;

pub mod priority;
pub mod thread;

/// The time quantum for the scheduler, in milliseconds.
pub const SCHEDULER_QUANTUM_MS: u32 = 1_000;

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
    current_thread: Box<Thread>,
    /// A local, atomic, priority for the current thread.
    current_priority: priority::AtomicPriority,
    should_exit_thread: AtomicBool,
    queues: Q,
}

impl<Q: priority::ThreadQueue> Scheduler<Q> {
    #[must_use]
    fn new(kernel_thread: thread::Thread) -> Self {
        let current_priority = priority::AtomicPriority::new(kernel_thread.priority());
        let root_proc = kernel_thread.process();

        Self {
            current_thread: Box::new(kernel_thread),
            current_priority,
            should_exit_thread: AtomicBool::new(false),
            queues: Q::create(root_proc),
        }
    }

    pub fn exit_current_thread(&self) {
        self.should_exit_thread.store(true, Ordering::Relaxed);
    }

    pub fn schedule_thread(&mut self, thread: Pin<Box<thread::Thread>>) {
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
        // It is tempting to use a spin loop here, but it is better to use the CPU for the last thread
        // than to waste it on a spin loop.
        // It is also a better solution if `yield` is implemented.
        if IN_RESCHEDULE
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        x86_64::instructions::interrupts::disable();

        // Swap the current thread with the next one.
        let mut new_thread = Pin::into_inner(self.queues.next());
        core::mem::swap(self.current_thread.as_mut(), &mut new_thread);
        let mut old_thread = new_thread; // Yes...

        // Gather information about the old thread.
        let old_priority = self
            .current_priority
            .swap(self.current_thread().priority(), Ordering::Relaxed);
        unsafe { old_thread.set_priority(old_priority) };
        let old_should_exit = self.should_exit_thread.swap(false, Ordering::Relaxed);

        // Handle stack pointers.
        let old_stack = old_thread.last_stack_ptr_mut();
        let new_stack = self.current_thread().last_stack_ptr();

        if old_should_exit {
            // FIXME: Handle this properly
            drop(old_thread);
        } else {
            self.queues.append(Pin::new(old_thread));
        }

        let cr3 = self.current_process().address_space().cr3_raw();

        IN_RESCHEDULE.store(false, Ordering::Release);

        // Safety:
        // Interrupts are indeed disabled at the start of the function.
        unsafe {
            crate::cpu::context::context_switch(old_stack, new_stack, cr3);
        }

        unreachable!("The scheduler should never return from a context switch.");
        // unsafe { core::hint::unreachable_unchecked() };
    }
}

pub fn reschedule() {
    // Safety:
    // Data races are avoided by the `Scheduler::reschedule` function.
    #[allow(static_mut_refs)]
    unsafe {
        SCHEDULER.as_mut().unwrap().reschedule();
    }
}

pub fn spawn_thread(thread: Pin<Box<Thread>>) {
    #[allow(static_mut_refs)]
    unsafe {
        SCHEDULER.as_mut().unwrap().schedule_thread(thread);
    };
}

pub fn set_scheduling(enable: bool) {
    use crate::cpu::apic::timer;

    locals!().lapic().try_with_locked(|lapic| {
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
