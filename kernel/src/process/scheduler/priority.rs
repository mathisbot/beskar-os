//! Manages the priority of processes.
//!
//! This helps the scheduler to decide which process to run next.
use super::thread::Thread;
use crate::process::Process;
use alloc::{boxed::Box, sync::Arc};
use core::{
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
};
use hyperdrive::queues::mpsc::MpscQueue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 1,
    Normal = 2,
    /// The thread should be scheduled as soon as possible.
    ///
    /// Suitable for real-time tasks.
    High = 3,
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
/// The `next` function must not allocate memory, acquire locks, ...
/// because it will be used by interrupt handlers.
pub unsafe trait ThreadQueue {
    fn append(&self, thread: Pin<Box<Thread>>);
    fn next(&self) -> Option<Pin<Box<Thread>>>;
}

pub struct RoundRobinQueues {
    current: AtomicUsize,
    cycle: [Priority; 6],
    low: MpscQueue<Thread>,
    normal: MpscQueue<Thread>,
    high: MpscQueue<Thread>,
}

impl RoundRobinQueues {
    pub fn new(root_proc: Arc<Process>) -> Self {
        Self {
            cycle: [
                Priority::High,
                Priority::Normal,
                Priority::High,
                Priority::Low,
                Priority::High,
                Priority::Normal,
            ],
            current: AtomicUsize::default(),
            low: MpscQueue::new(Box::pin(Thread::new_stub(root_proc.clone()))),
            normal: MpscQueue::new(Box::pin(Thread::new_stub(root_proc.clone()))),
            high: MpscQueue::new(Box::pin(Thread::new_stub(root_proc))),
        }
    }

    /// Cycle through the priorities.
    #[inline]
    fn cycle_priority(&self) -> Priority {
        let current = self.current.fetch_add(1, Ordering::Relaxed);
        self.cycle[current % self.cycle.len()]
    }
}

unsafe impl ThreadQueue for RoundRobinQueues {
    fn append(&self, thread: Pin<Box<Thread>>) {
        match thread.priority() {
            Priority::Low => {
                self.low.enqueue(thread);
            }
            Priority::Normal => {
                self.normal.enqueue(thread);
            }
            Priority::High => {
                self.high.enqueue(thread);
            }
        }
    }

    fn next(&self) -> Option<Pin<Box<Thread>>> {
        match self.cycle_priority() {
            Priority::Low => self.low.dequeue(),
            Priority::Normal => self.normal.dequeue(),
            Priority::High => self.high.dequeue(),
        }
    }
}
