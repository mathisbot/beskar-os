#![no_std]

extern crate alloc;

use alloc::{
    collections::{binary_heap::BinaryHeap, vec_deque::VecDeque},
    vec::Vec,
};
use beskar_core::{debug_panic, process::SleepHandle, time::Instant};
use core::{cmp::Reverse, hash::Hash};
use hashbrown::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitRequest {
    pub event: Option<SleepHandle>,
    pub deadline: Option<Instant>,
}

impl WaitRequest {
    #[must_use]
    #[inline]
    pub const fn new(event: Option<SleepHandle>, deadline: Option<Instant>) -> Self {
        debug_assert!(
            event.is_some() || deadline.is_some(),
            "creating wait request with no event or deadline"
        );
        Self { event, deadline }
    }

    #[must_use]
    #[inline]
    pub const fn indefinite() -> Self {
        Self {
            event: None,
            deadline: None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn event(handle: SleepHandle) -> Self {
        Self {
            event: Some(handle),
            deadline: None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn until(deadline: Instant) -> Self {
        Self {
            event: None,
            deadline: Some(deadline),
        }
    }

    #[must_use]
    #[inline]
    pub const fn event_or_timeout(handle: SleepHandle, deadline: Instant) -> Self {
        Self {
            event: Some(handle),
            deadline: Some(deadline),
        }
    }
}

/// Kernel-owned contract needed by the generic wait database.
///
/// The wait crate owns the state machine and wake routing, while kernel code
/// provides thread-specific state transitions through this trait.
pub trait WaitableThread {
    type Tid: Copy + Ord + Eq + Hash;

    fn tid(&self) -> Self::Tid;
    fn mark_blocked(&mut self);
    fn finish_wait(&mut self, wake_result: WakeResult);
}

/// Result of committing a running thread into the wait database.
pub enum BlockCommit<T> {
    /// Thread is now blocked and owned by the wait database.
    Parked,
    /// Thread must stay runnable (woken during parking, cancelled, or deferred commit).
    Requeue(T),
}

pub enum WakeHit<T> {
    None,
    WokeParking,
    Ready(T),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct TimerKey<Tid> {
    deadline: Instant,
    tid: Tid,
    token: u64,
}

#[derive(Clone, Copy)]
struct WaitNode<Tid> {
    tid: Tid,
    token: u64,
}

struct WaitSlot<T: WaitableThread> {
    token: u64,
    request: WaitRequest,
    state: SlotState<T>,
}

enum SlotState<T: WaitableThread> {
    Parking { wake_result: Option<WakeResult> },
    Blocked { thread: T },
}

pub struct WaitDb<T: WaitableThread> {
    slots: HashMap<T::Tid, WaitSlot<T>>,
    timers: BinaryHeap<Reverse<TimerKey<T::Tid>>>,
    events: HashMap<SleepHandle, VecDeque<WaitNode<T::Tid>>>,
    next_token: u64,
}

impl<T: WaitableThread> WaitDb<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            timers: BinaryHeap::new(),
            events: HashMap::new(),
            next_token: 1,
        }
    }

    #[must_use]
    pub const fn alloc_token(&mut self) -> u64 {
        let token = self.next_token;
        self.next_token += 1;
        token
    }

    #[must_use]
    pub fn register_wait(&mut self, tid: T::Tid, request: WaitRequest) -> u64 {
        if let Some(existing) = self.slots.get(&tid) {
            debug_panic!("tried to register wait while already waiting");
            return existing.token;
        }

        let token = self.alloc_token();
        self.slots.insert(
            tid,
            WaitSlot {
                token,
                request,
                state: SlotState::Parking { wake_result: None },
            },
        );

        if let Some(handle) = request.event {
            self.events
                .entry(handle)
                .or_default()
                .push_back(WaitNode { tid, token });
        }

        if let Some(deadline) = request.deadline {
            self.timers.push(Reverse(TimerKey {
                deadline,
                tid,
                token,
            }));
        }

        token
    }

    /// Commit a parking thread to blocked state.
    ///
    /// If the thread was already woken while parking, returns it as runnable.
    #[must_use]
    pub fn finalize_block(&mut self, token: u64, mut thread: T) -> BlockCommit<T> {
        let tid = thread.tid();

        let Some(mut slot) = self.slots.remove(&tid) else {
            thread.finish_wait(WakeResult::cancelled());
            return BlockCommit::Requeue(thread);
        };

        if slot.token != token {
            if matches!(slot.state, SlotState::Blocked { .. }) {
                // Should be unreachable in correct flow; preserve DB ownership.
                self.slots.insert(tid, slot);
                thread.finish_wait(WakeResult::cancelled());
                return BlockCommit::Requeue(thread);
            }

            self.cleanup_membership(tid, &slot);
            thread.finish_wait(WakeResult::cancelled());
            return BlockCommit::Requeue(thread);
        }

        match slot.state {
            SlotState::Parking { wake_result } => {
                if let Some(wake_result) = wake_result {
                    self.cleanup_membership(tid, &slot);
                    thread.finish_wait(wake_result);
                    return BlockCommit::Requeue(thread);
                }

                thread.mark_blocked();
                slot.state = SlotState::Blocked { thread };
                self.slots.insert(tid, slot);
                BlockCommit::Parked
            }
            SlotState::Blocked { .. } => {
                debug_panic!("duplicate finalize_block for blocked thread");
                // Preserve DB ownership to avoid dropping blocked thread.
                self.slots.insert(tid, slot);
                thread.finish_wait(WakeResult::cancelled());
                BlockCommit::Requeue(thread)
            }
        }
    }

    #[must_use]
    pub fn collect_timed_out(&mut self, now: Instant) -> Vec<T> {
        let mut ready = Vec::new();

        while let Some(Reverse(timer)) = self.timers.peek()
            && timer.deadline <= now
        {
            let tid = timer.tid;
            let token = timer.token;
            self.timers.pop();

            match self.wake_by_token(tid, token, WakeResult::new(WakeCause::Timeout, 0)) {
                WakeHit::Ready(thread) => ready.push(thread),
                WakeHit::WokeParking | WakeHit::None => {}
            }
        }

        ready
    }

    #[must_use]
    pub fn wake_event_single(&mut self, handle: SleepHandle) -> WakeHit<T> {
        loop {
            let node = {
                let Some(queue) = self.events.get_mut(&handle) else {
                    return WakeHit::None;
                };
                queue.pop_front()
            };

            let Some(node) = node else {
                self.events.remove(&handle);
                return WakeHit::None;
            };

            let wake = self.wake_by_token(
                node.tid,
                node.token,
                WakeResult::new(WakeCause::Event, handle.raw()),
            );

            let should_remove = self.events.get(&handle).is_some_and(VecDeque::is_empty);
            if should_remove {
                self.events.remove(&handle);
            }

            if !matches!(wake, WakeHit::None) {
                return wake;
            }
        }
    }

    #[must_use]
    pub fn wake_event_all(&mut self, handle: SleepHandle) -> Vec<T> {
        let mut ready = Vec::new();

        if let Some(mut queue) = self.events.remove(&handle) {
            while let Some(node) = queue.pop_front() {
                if let WakeHit::Ready(thread) = self.wake_by_token(
                    node.tid,
                    node.token,
                    WakeResult::new(WakeCause::Event, handle.raw()),
                ) {
                    ready.push(thread);
                }
            }
        }

        ready
    }

    #[must_use]
    pub fn wake_thread(&mut self, tid: T::Tid, wake_result: WakeResult) -> WakeHit<T> {
        let Some(mut slot) = self.slots.remove(&tid) else {
            return WakeHit::None;
        };
        let slot_token = slot.token;
        let slot_event = slot.request.event;

        match slot.state {
            SlotState::Parking {
                wake_result: parked,
            } => {
                if parked.is_some() {
                    self.slots.insert(tid, slot);
                    return WakeHit::None;
                }

                if let Some(handle) = slot_event {
                    self.remove_event_membership(handle, tid, slot_token);
                }

                slot.state = SlotState::Parking {
                    wake_result: Some(wake_result),
                };
                self.slots.insert(tid, slot);
                WakeHit::WokeParking
            }
            SlotState::Blocked { mut thread } => {
                if let Some(handle) = slot_event {
                    self.remove_event_membership(handle, tid, slot_token);
                }
                thread.finish_wait(wake_result);
                WakeHit::Ready(thread)
            }
        }
    }

    fn wake_by_token(&mut self, tid: T::Tid, token: u64, wake_result: WakeResult) -> WakeHit<T> {
        let Some(mut slot) = self.slots.remove(&tid) else {
            return WakeHit::None;
        };

        if slot.token != token {
            self.slots.insert(tid, slot);
            return WakeHit::None;
        }
        let slot_event = slot.request.event;

        match slot.state {
            SlotState::Parking {
                wake_result: parked,
            } => {
                if parked.is_some() {
                    self.slots.insert(tid, slot);
                    return WakeHit::None;
                }

                if let Some(handle) = slot_event {
                    self.remove_event_membership(handle, tid, token);
                }

                slot.state = SlotState::Parking {
                    wake_result: Some(wake_result),
                };
                self.slots.insert(tid, slot);
                WakeHit::WokeParking
            }
            SlotState::Blocked { mut thread } => {
                if let Some(handle) = slot_event {
                    self.remove_event_membership(handle, tid, token);
                }
                thread.finish_wait(wake_result);
                WakeHit::Ready(thread)
            }
        }
    }

    #[inline]
    fn cleanup_membership(&mut self, tid: T::Tid, slot: &WaitSlot<T>) {
        if let Some(handle) = slot.request.event {
            self.remove_event_membership(handle, tid, slot.token);
        }
    }

    fn remove_event_membership(&mut self, handle: SleepHandle, tid: T::Tid, token: u64) {
        if let Some(queue) = self.events.get_mut(&handle) {
            if let Some(pos) = queue
                .iter()
                .position(|node| node.tid == tid && node.token == token)
            {
                queue.remove(pos);
            }

            if queue.is_empty() {
                self.events.remove(&handle);
            }
        }
    }
}

impl<T: WaitableThread> Default for WaitDb<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeCause {
    None,
    Event,
    Timeout,
    Cancelled,
    Killed,
}

impl From<WakeCause> for beskar_core::process::WaitResult {
    fn from(value: WakeCause) -> Self {
        match value {
            WakeCause::None => Self::Unknown,
            WakeCause::Event => Self::Event,
            WakeCause::Timeout => Self::Timeout,
            WakeCause::Cancelled => Self::Cancelled,
            WakeCause::Killed => Self::Killed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeResult {
    cause: WakeCause,
    /// For event wakeups this stores the source key/handle.
    source: u64,
}

impl WakeResult {
    #[must_use]
    #[inline]
    pub const fn none() -> Self {
        Self {
            cause: WakeCause::None,
            source: 0,
        }
    }

    #[must_use]
    #[inline]
    pub const fn cancelled() -> Self {
        Self {
            cause: WakeCause::Cancelled,
            source: 0,
        }
    }

    #[must_use]
    #[inline]
    pub const fn new(cause: WakeCause, source: u64) -> Self {
        Self { cause, source }
    }

    #[must_use]
    #[inline]
    pub const fn cause(self) -> WakeCause {
        self.cause
    }

    #[must_use]
    #[inline]
    pub const fn source(self) -> u64 {
        self.source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct FakeThread {
        tid: u64,
        blocked: bool,
        wake: WakeResult,
    }

    impl FakeThread {
        fn new(tid: u64) -> Self {
            Self {
                tid,
                blocked: false,
                wake: WakeResult::none(),
            }
        }
    }

    impl WaitableThread for FakeThread {
        type Tid = u64;

        fn tid(&self) -> Self::Tid {
            self.tid
        }

        fn mark_blocked(&mut self) {
            self.blocked = true;
        }

        fn finish_wait(&mut self, wake_result: WakeResult) {
            self.blocked = false;
            self.wake = wake_result;
        }
    }

    #[test]
    fn block_then_wake_thread_returns_ready() {
        let mut db = WaitDb::<FakeThread>::new();
        let tid = 1_u64;
        let token = db.register_wait(tid, WaitRequest::indefinite());

        let thread = FakeThread::new(tid);
        assert!(matches!(
            db.finalize_block(token, thread),
            BlockCommit::Parked
        ));

        let wake = db.wake_thread(tid, WakeResult::new(WakeCause::Killed, 0xAA));
        let WakeHit::Ready(thread) = wake else {
            panic!("blocked waiter should become ready");
        };

        assert!(!thread.blocked);
        assert_eq!(thread.wake.cause(), WakeCause::Killed);
        assert_eq!(thread.wake.source(), 0xAA);
    }

    #[test]
    fn wake_while_parking_then_finalize_requeues_with_wake_result() {
        let mut db = WaitDb::<FakeThread>::new();
        let tid = 2_u64;
        let handle = SleepHandle::from_raw(0x42);
        let token = db.register_wait(tid, WaitRequest::event(handle));

        let wake = db.wake_event_single(handle);
        assert!(matches!(wake, WakeHit::WokeParking));

        let thread = FakeThread::new(tid);
        let BlockCommit::Requeue(thread) = db.finalize_block(token, thread) else {
            panic!("parking wake should requeue at finalize_block");
        };

        assert_eq!(thread.wake.cause(), WakeCause::Event);
        assert_eq!(thread.wake.source(), handle.raw());
    }

    #[test]
    fn timeout_marks_parking_then_finalize_requeues_timeout() {
        let mut db = WaitDb::<FakeThread>::new();
        let tid = 3_u64;
        let deadline = Instant::from_micros(100);
        let token = db.register_wait(tid, WaitRequest::until(deadline));

        let ready = db.collect_timed_out(deadline);
        assert!(ready.is_empty());

        let thread = FakeThread::new(tid);
        let BlockCommit::Requeue(thread) = db.finalize_block(token, thread) else {
            panic!("timed out parking thread should requeue on finalize");
        };

        assert_eq!(thread.wake.cause(), WakeCause::Timeout);
        assert_eq!(thread.wake.source(), 0);
    }

    #[test]
    fn wake_event_all_wakes_all_blocked_waiters() {
        let mut db = WaitDb::<FakeThread>::new();
        let handle = SleepHandle::from_raw(0x77);

        let tid_a = 10_u64;
        let token_a = db.register_wait(tid_a, WaitRequest::event(handle));
        let tid_b = 11_u64;
        let token_b = db.register_wait(tid_b, WaitRequest::event(handle));

        assert!(matches!(
            db.finalize_block(token_a, FakeThread::new(tid_a)),
            BlockCommit::Parked
        ));
        assert!(matches!(
            db.finalize_block(token_b, FakeThread::new(tid_b)),
            BlockCommit::Parked
        ));

        let mut ready = db.wake_event_all(handle);
        assert_eq!(ready.len(), 2);

        ready.sort_by_key(|t| t.tid);
        assert_eq!(ready[0].tid, tid_a);
        assert_eq!(ready[1].tid, tid_b);
        assert_eq!(ready[0].wake.cause(), WakeCause::Event);
        assert_eq!(ready[1].wake.cause(), WakeCause::Event);
        assert_eq!(ready[0].wake.source(), handle.raw());
        assert_eq!(ready[1].wake.source(), handle.raw());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "tried to register wait while already waiting")]
    fn duplicate_register_panics_in_debug() {
        let mut db = WaitDb::<FakeThread>::new();
        let tid = 99_u64;

        let _ = db.register_wait(tid, WaitRequest::indefinite());
        let _ = db.register_wait(tid, WaitRequest::event(SleepHandle::from_raw(1)));
    }
}
