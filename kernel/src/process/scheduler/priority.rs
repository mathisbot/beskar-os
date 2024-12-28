//! Manages the priority of processes.
//!
//! This helps the scheduler to decide which process to run next.

use core::sync::atomic::{AtomicU8, Ordering};

use alloc::{boxed::Box, collections::vec_deque::VecDeque};

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
    fn append(&mut self, thread: Thread);
    fn next(&mut self) -> Thread;
}

pub struct RoundRobinQueues {
    current: usize,
    cycle: Box<[Priority]>,
    low: VecDeque<Thread>,
    normal: VecDeque<Thread>,
    high: VecDeque<Thread>,
}

impl Default for RoundRobinQueues {
    fn default() -> Self {
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
            low: VecDeque::default(),
            normal: VecDeque::default(),
            high: VecDeque::default(),
        }
    }
}

unsafe impl ThreadQueue for RoundRobinQueues {
    /// Appends a thread to the appropriate queue
    ///
    /// This function can allocate memory (when the inner `VecDeque` grows).
    fn append(&mut self, thread: Thread) {
        match thread.priority() {
            Priority::Null => {}
            Priority::Low => {
                self.low.push_back(thread);
            }
            Priority::Normal => {
                self.normal.push_back(thread);
            }
            Priority::High => {
                self.high.push_back(thread);
            }
        }
    }

    #[must_use]
    /// Returns the next thread to run
    ///
    /// This function does not use locks (this implies no memory allocations).
    fn next(&mut self) -> Thread {
        let priority = self.cycle[self.current];

        self.current = (self.current + 1) % self.cycle.len();

        let next_thread = match priority {
            Priority::Null => unreachable!(),
            Priority::Low => self.low.pop_front(),
            Priority::Normal => self.normal.pop_front(),
            Priority::High => self.high.pop_front(),
        };

        // This line can be dangerous (infinite recursion).
        // We must ensure that at least one thread is in the queues.
        // This should hold because the kernel thread is always in the queues.
        // FIXME: Find a better way to write this.
        next_thread.unwrap_or_else(|| self.next())
    }
}
