use core::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
};

use alloc::{boxed::Box, sync::Arc};
use priority::ThreadQueue;
use thread::Thread;

use crate::locals;

use super::Process;

pub mod priority;
pub mod thread;

/// The time quantum for the scheduler, in milliseconds.
///
/// According to the Internet, Windows uses 20-60ms, Linux uses 0.75-6ms.
pub const SCHEDULER_QUANTUM_MS: u32 = 60;

// TODO: Runtime size for schedulers
// Currently, it takes 4KiB of memory but on a vast majority of systems, it only needs a few schedulers.
// 
// Because scheduler will be playing with context switching, we cannot acquire locks.
// Therefore, we will have to use unsafe mutable statics, in combination with `AtomicBool`s.
static mut SCHEDULERS: [Option<Scheduler>; 256] = [const { None }; 256];
static mut QUEUE: Option<priority::RoundRobinQueues> = None;

/// This function initializes the scheduler with the kernel thread.
///
/// ## Safety
///
/// This function should only be called once, and only by the kernel, with the kernel thread.
pub unsafe fn init(kernel_thread: thread::Thread) {
    if locals!().core_id() == 0 {
        #[allow(static_mut_refs)]
        unsafe {
            QUEUE = Some(priority::RoundRobinQueues::create(kernel_thread.process()));
        }
    }

    let scheduler = Scheduler::new(kernel_thread);
    // Safety:
    // Function safety guards.
    unsafe {
        SCHEDULERS[locals!().core_id()] = Some(scheduler);
    }
}

#[derive(Debug, Clone, Copy)]
struct ContextSwitch {
    old_stack: *mut usize,
    new_stack: *const usize,
    cr3: usize,
}

impl ContextSwitch {
    #[inline]
    /// Performs the context switch.
    ///
    /// ## Safety
    ///
    /// See `kernel::cpu::context::context_switch`.
    unsafe fn perform(&self) {
        unsafe { crate::cpu::context::context_switch(self.old_stack, self.new_stack, self.cr3) };
    }
}

pub struct Scheduler {
    current_thread: Box<Thread>,
    /// A local, atomic, priority for the current thread.
    current_priority: priority::AtomicPriority,
    should_exit_thread: AtomicBool,
}

impl Scheduler {
    #[must_use]
    fn new(kernel_thread: thread::Thread) -> Self {
        let current_priority = priority::AtomicPriority::new(kernel_thread.priority());

        Self {
            current_thread: Box::new(kernel_thread),
            current_priority,
            should_exit_thread: AtomicBool::new(false),
        }
    }

    pub fn exit_current_thread(&self) {
        self.should_exit_thread.store(true, Ordering::Relaxed);
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

    #[inline]
    pub fn change_current_thread_priority(&self, new_priority: priority::Priority) {
        self.current_priority.store(new_priority, Ordering::Relaxed);
    }

    #[must_use]
    /// Changes the internal state of the scheduler to the next thread.
    ///
    /// This function does not change the context, but will disable interrupts
    /// if scheduling was successful.
    fn reschedule(&mut self) -> Option<ContextSwitch> {
        static IN_RESCHEDULE: AtomicBool = AtomicBool::new(false);

        // We cannot acquire locks, so we imitate one with an `AtomicBool`.
        // It is tempting to use a spin loop here, but it is better to use the CPU for the last thread
        // than to waste it on a spin loop.
        // It is also a better solution if `yield` is implemented.
        if IN_RESCHEDULE
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // FIXME: This solution is not optimal for multiple cores:
            // As AP start at the same time, they will all try to reschedule at the same time.
            // Only one AP will then be able to reschedule.
            return None;
        }

        x86_64::instructions::interrupts::disable();

        // Swap the current thread with the next one.
        let mut new_thread = Pin::into_inner(
            #[allow(static_mut_refs)]
            if let Some(new_thread) = unsafe { QUEUE.as_mut() }.unwrap().next() {
                new_thread
            } else {
                IN_RESCHEDULE.store(false, Ordering::Release);
                x86_64::instructions::interrupts::enable();
                return None;
            },
        );
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
            // As the scheduler must not acquire locks, it cannot drop heap-allocated memory.
            // For now, we will just forget the thread.
            // Maybe creating a cleaning thread?
            core::mem::forget(old_thread);
        } else {
            #[allow(static_mut_refs)]
            unsafe { QUEUE.as_mut() }
                .unwrap()
                .append(Pin::new(old_thread));
        }

        let cr3 = self.current_process().address_space().cr3_raw();

        IN_RESCHEDULE.store(false, Ordering::Release);

        Some(ContextSwitch {
            old_stack,
            new_stack,
            cr3,
        })
    }
}

/// Reschedules the scheduler.
///
/// ## Safety
///
/// This function must only be called inside of the timer interrupt handler,
/// and EOI is sent to the APIC in the function.
pub(crate) unsafe fn reschedule() {
    // Safety:
    // Interrupts are disabled at the start of the function.
    // Data races are avoided by the `Scheduler::reschedule` function.
    // FIXME: Find a workaround for static mutable references.
    #[allow(static_mut_refs)]
    let rescheduling_result = unsafe {
        SCHEDULERS[locals!().core_id()]
            .as_mut()
            .unwrap()
            .reschedule()
    };

    // Safety:
    // We are only writing a single `u32` to MMIO.
    // Also, APIC is initialized if the scheduler is initialized.
    unsafe { locals!().lapic().force_lock() }.send_eoi();

    if let Some(context_switch) = rescheduling_result {
        // Safety:
        // Interrupts are indeed disabled at the start of the function.
        unsafe { context_switch.perform() };
    }
}

#[must_use]
/// Returns the current process.
///
/// ## Safety
///
/// Scheduling must be disabled.
// TODO: Process tree?
pub unsafe fn current_process() -> Arc<Process> {
    // FIXME: Find a workaround for static mutable references.
    #[allow(static_mut_refs)]
    unsafe {
        SCHEDULERS[locals!().core_id()]
            .as_mut()
            .unwrap()
            .current_process()
    }
}

pub fn spawn_thread(thread: Pin<Box<Thread>>) {
    // FIXME: Find a workaround for static mutable references.
    #[allow(static_mut_refs)]
    unsafe {
        QUEUE.as_mut().unwrap().append(thread);
    };
}

/// Sets the scheduling of the scheduler.
///
/// What this function really does is enabling the timer interrupt.
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

pub fn change_current_thread_priority(priority: priority::Priority) {
    #[allow(static_mut_refs)]
    unsafe { SCHEDULERS[locals!().core_id()].as_ref() }
        .unwrap()
        .change_current_thread_priority(priority);
}

/// Exits the current thread.
/// 
/// ## Safety
/// 
/// The context will be brutally switched without returning.
/// If any locks are acquired, they will be poisoned.
pub unsafe fn exit_current_thread() {
    #[allow(static_mut_refs)]
    unsafe { SCHEDULERS[locals!().core_id()].as_ref() }
        .unwrap()
        .exit_current_thread();
    // Wait for the next thread to be scheduled.
    loop {
        core::hint::spin_loop();
    }
}
