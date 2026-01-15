//! Manages the priority of processes.
//!
//! This helps the scheduler to decide which process to run next.
use super::thread::Thread;
use crate::process::Process;
use alloc::{boxed::Box, sync::Arc};
use hyperdrive::queues::mpsc::MpscQueue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Idle = 0,
    Low = 1,
    Normal = 2,
    /// The thread should be scheduled as soon as possible.
    ///
    /// Suitable for real-time tasks.
    High = 3,
    Realtime = 4,
}

impl TryFrom<u8> for Priority {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Low),
            2 => Ok(Self::Normal),
            3 => Ok(Self::High),
            _ => Err(()),
        }
    }
}

impl From<Priority> for u8 {
    #[inline]
    fn from(priority: Priority) -> Self {
        priority as Self
    }
}

/// A trait for managing thread queues.
///
/// # Safety
///
/// The `pop_best` and `should_switch` functions must not allocate memory, acquire locks, ...
/// because they will be used by interrupt handlers.
pub unsafe trait ThreadQueue {
    fn append(&self, thread: Box<Thread>);
    /// Returns the best thread to run next, or None if no runnable threads are available.
    // #[expect(clippy::unnecessary_box_returns, reason = "Thread objects are large")]
    fn pop_best(&self) -> Option<Box<Thread>>;
    /// Determines whether we should switch from the current thread to the candidate thread.
    ///
    /// Returns `true` if a context switch is beneficial, `false` if the current thread should
    /// keep running.
    fn should_switch(
        &self,
        current: &Thread,
        candidate: &Thread,
        reason: super::RescheduleReason,
    ) -> bool;
}

pub struct RoundRobinQueues {
    idle: MpscQueue<Thread>,
    low: MpscQueue<Thread>,
    normal: MpscQueue<Thread>,
    high: MpscQueue<Thread>,
    realtime: MpscQueue<Thread>,
}

impl RoundRobinQueues {
    pub fn new(root_proc: Arc<Process>) -> Self {
        Self {
            low: MpscQueue::new(Box::new(Thread::new_stub(root_proc.clone()))),
            normal: MpscQueue::new(Box::new(Thread::new_stub(root_proc.clone()))),
            high: MpscQueue::new(Box::new(Thread::new_stub(root_proc.clone()))),
            idle: MpscQueue::new(Box::new(Thread::new_stub(root_proc.clone()))),
            realtime: MpscQueue::new(Box::new(Thread::new_stub(root_proc))),
        }
    }
}

unsafe impl ThreadQueue for RoundRobinQueues {
    fn append(&self, thread: Box<Thread>) {
        match thread.priority() {
            Priority::Idle => {
                self.idle.enqueue(thread);
            }
            Priority::Low => {
                self.low.enqueue(thread);
            }
            Priority::Normal => {
                self.normal.enqueue(thread);
            }
            Priority::High => {
                self.high.enqueue(thread);
            }
            Priority::Realtime => {
                self.realtime.enqueue(thread);
            }
        }
    }

    fn pop_best(&self) -> Option<Box<Thread>> {
        // Try each queue in order of priority
        for queue in [&self.realtime, &self.high, &self.normal, &self.low] {
            if let Some(thread) = queue.dequeue() {
                return Some(thread);
            }
        }

        // Finally, try idle
        self.idle.dequeue()
    }

    fn should_switch(
        &self,
        current: &Thread,
        candidate: &Thread,
        reason: super::RescheduleReason,
    ) -> bool {
        let cand_prio = candidate.priority();
        let curr_prio = current.priority();

        if cand_prio > curr_prio {
            return true;
        }
        if cand_prio < curr_prio {
            return false;
        }

        // Same priority: only rotate if not idle and reason permits
        cand_prio != Priority::Idle && reason.should_rotate()
    }
}
