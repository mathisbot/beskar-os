use super::Thread;
use crate::process::scheduler::thread::ThreadId;
use alloc::{
    boxed::Box,
    collections::{binary_heap::BinaryHeap, btree_map::BTreeMap, vec_deque::VecDeque},
    vec::Vec,
};
use beskar_core::{
    process::{SleepHandle, SleepReason},
    time::Instant,
};
use core::cmp::Reverse;

struct Sleeper {
    reason: SleepReason,
    thread: Box<Thread>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimerKey {
    deadline: Instant,
    tid: ThreadId,
}

impl Ord for TimerKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.deadline
            .cmp(&other.deadline)
            .then(self.tid.cmp(&other.tid))
    }
}

impl PartialOrd for TimerKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct SleepQueues {
    sleepers: BTreeMap<ThreadId, Sleeper>,
    timers: BinaryHeap<Reverse<TimerKey>>, // Min-heap via Reverse.
    events: BTreeMap<SleepHandle, VecDeque<ThreadId>>,
    indefinite: Vec<ThreadId>,
}

impl SleepQueues {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            sleepers: BTreeMap::new(),
            timers: BinaryHeap::new(),
            events: BTreeMap::new(),
            indefinite: Vec::new(),
        }
    }

    pub fn insert(&mut self, reason: SleepReason, mut thread: Box<Thread>) {
        let tid = thread.id();
        if let Some(deadline) = reason.deadline() {
            thread.stats_mut().wake_time = deadline;
        } else {
            thread.stats_mut().wake_time = Instant::ZERO;
        }

        self.sleepers.insert(tid, Sleeper { reason, thread });

        match reason {
            SleepReason::Until(deadline) => {
                self.timers.push(Reverse(TimerKey { deadline, tid }));
            }
            SleepReason::Event(handle) => {
                self.events.entry(handle).or_default().push_back(tid);
            }
            SleepReason::Indefinite => self.indefinite.push(tid),
        }
    }

    pub fn pop_ready(&mut self, now: Instant) -> Option<Box<Thread>> {
        loop {
            let Reverse(timer) = self.timers.peek()?;
            if timer.deadline > now {
                return None;
            }

            let Reverse(timer) = self.timers.pop()?;
            if let Some(sleeper) = self.sleepers.remove(&timer.tid) {
                return Some(sleeper.thread);
            }
        }
    }

    pub fn wake_event_single(&mut self, handle: SleepHandle) -> Option<Box<Thread>> {
        let tids = self.events.get_mut(&handle)?;

        if let Some(tid) = tids.pop_front()
            && let Some(sleeper) = self.sleepers.remove(&tid)
        {
            if tids.is_empty() {
                self.events.remove(&handle);
            }
            Some(sleeper.thread)
        } else {
            None
        }
    }

    pub fn wake_event_all(&mut self, handle: SleepHandle) -> Vec<Box<Thread>> {
        let mut ready = Vec::new();

        if let Some(mut tids) = self.events.remove(&handle) {
            while let Some(tid) = tids.pop_front() {
                if let Some(sleeper) = self.sleepers.remove(&tid) {
                    ready.push(sleeper.thread);
                }
            }
        }

        ready
    }

    pub fn wake_thread(&mut self, tid: ThreadId) -> Option<Box<Thread>> {
        let sleeper = self.sleepers.remove(&tid)?;

        match sleeper.reason {
            SleepReason::Event(handle) => {
                if let Some(queue) = self.events.get_mut(&handle)
                    && let Some(pos) = queue.iter().position(|id| *id == tid)
                {
                    queue.remove(pos);
                    if queue.is_empty() {
                        self.events.remove(&handle);
                    }
                }
            }
            SleepReason::Indefinite => {
                if let Some(pos) = self.indefinite.iter().position(|id| *id == tid) {
                    self.indefinite.remove(pos);
                }
            }
            SleepReason::Until(_) => {
                // The timer heap may still contain an entry; it will be skipped once popped.
            }
        }

        Some(sleeper.thread)
    }
}
