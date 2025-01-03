//! Manages the priority of processes.
//!
//! This helps the scheduler to decide which process to run next.

use core::{
    pin::Pin,
    sync::atomic::{AtomicU8, Ordering},
};

use alloc::{boxed::Box, sync::Arc};
use hyperdrive::queues::mpsc::MpscQueue;

use crate::process::Process;

use super::thread::Thread;

#[derive(Debug)]
pub struct AtomicPriority(AtomicU8);

impl AtomicPriority {
    #[must_use]
    pub const fn new(priority: Priority) -> Self {
        Self(AtomicU8::new(priority as u8))
    }

    #[must_use]
    pub fn load(&self, order: Ordering) -> Priority {
        self.0.load(order).try_into().unwrap()
    }

    pub fn store(&self, priority: Priority, order: Ordering) {
        self.0.store(priority.into(), order);
    }

    pub fn swap(&self, priority: Priority, order: Ordering) -> Priority {
        self.0.swap(priority.into(), order).try_into().unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// The thread should not be scheduled.
    Null = 0,
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
            0 => Ok(Self::Null),
            1 => Ok(Self::Low),
            2 => Ok(Self::Normal),
            3 => Ok(Self::High),
            _ => Err(()),
        }
    }
}

impl From<Priority> for u8 {
    fn from(priority: Priority) -> Self {
        priority as Self
    }
}

/// A trait for managing thread queues.
///
/// ## Safety
///
/// The `next` function must not allocate memory, acquire locks, ...
/// because it will be used by interrupt handlers.
pub unsafe trait ThreadQueue {
    fn create(root_proc: Arc<Process>) -> Self;
    fn append(&mut self, thread: Pin<Box<Thread>>);
    fn next(&mut self) -> Pin<Box<Thread>>;
}

pub struct RoundRobinQueues {
    current: usize,
    cycle: Box<[Priority]>,
    low: MpscQueue<Thread>,
    normal: MpscQueue<Thread>,
    high: MpscQueue<Thread>,
}

unsafe impl ThreadQueue for RoundRobinQueues {
    fn create(root_proc: Arc<Process>) -> Self {
        Self {
            cycle: alloc::vec![
                Priority::High,
                Priority::Normal,
                Priority::High,
                Priority::Low,
                Priority::High,
                Priority::Normal,
            ]
            .into_boxed_slice(),
            current: usize::default(),
            low: MpscQueue::new(Box::pin(Thread::new_stub(root_proc.clone()))),
            normal: MpscQueue::new(Box::pin(Thread::new_stub(root_proc.clone()))),
            high: MpscQueue::new(Box::pin(Thread::new_stub(root_proc))),
        }
    }

    fn append(&mut self, thread: Pin<Box<Thread>>) {
        match thread.priority() {
            Priority::Null => {}
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

    #[must_use]
    fn next(&mut self) -> Pin<Box<Thread>> {
        let priority = self.cycle[self.current];

        self.current = (self.current + 1) % self.cycle.len();

        let next_thread = match priority {
            Priority::Null => unreachable!(),
            Priority::Low => self.low.dequeue(),
            Priority::Normal => self.normal.dequeue(),
            Priority::High => self.high.dequeue(),
        };

        next_thread.unwrap_or_else(|| self.next())
    }
}
