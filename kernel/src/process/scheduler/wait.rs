use super::thread::{Thread, ThreadId, ThreadState};
use ::wait::{BlockCommit, WaitDb, WaitRequest, WaitableThread, WakeHit, WakeResult};
use alloc::{boxed::Box, vec::Vec};
use beskar_core::{process::SleepHandle, time::Instant};
use beskar_hal::instructions::without_interrupts;
use hyperdrive::locks::mcs::{MUMcsLock, McsNode};

/// Global wait database lock.
static WAIT_DB: MUMcsLock<WaitDb<Box<Thread>>> = MUMcsLock::uninit();

/// Initialize the wait database.
///
/// Must be called once during scheduler initialization.
pub fn init() {
    WAIT_DB.init(WaitDb::new());
}

/// Register a waiting request for the given thread and return its wait token.
#[must_use]
pub fn register_wait(tid: ThreadId, request: WaitRequest) -> u64 {
    with_dispatch_db(|db| db.register_wait(tid, request))
}

/// Try to commit a preempted thread into blocked state.
///
/// On lock contention, this returns Requeue so the scheduler can retry later
/// without fabricating wake reasons.
#[must_use]
pub fn commit_block(thread: Box<Thread>) -> BlockCommit<Box<Thread>> {
    let token = thread.wait_token();
    debug_assert_ne!(token, 0, "commit_block called with no armed wait token");

    let mut node = McsNode::new();
    let guard = without_interrupts(|| WAIT_DB.try_lock(&mut node));

    if let Some(mut guard) = guard {
        guard.finalize_block(token, thread)
    } else {
        BlockCommit::Requeue(thread)
    }
}

/// Collect all timeout-expired waiters that are ready to run.
///
/// Returns an empty vector if the database lock is contended.
#[must_use]
#[inline]
pub fn collect_timed_out(now: Instant) -> Vec<Box<Thread>> {
    try_with_dispatch_db(|db| db.collect_timed_out(now)).unwrap_or_default()
}

#[inline]
pub fn wake_event_single(handle: SleepHandle) -> WakeHit<Box<Thread>> {
    with_dispatch_db(|db| db.wake_event_single(handle))
}

#[inline]
pub fn wake_event_all(handle: SleepHandle) -> Vec<Box<Thread>> {
    with_dispatch_db(|db| db.wake_event_all(handle))
}

#[inline]
pub fn wake_thread(tid: ThreadId, wake_result: WakeResult) -> WakeHit<Box<Thread>> {
    with_dispatch_db(|db| db.wake_thread(tid, wake_result))
}

#[inline]
fn with_dispatch_db<R, F: FnOnce(&mut WaitDb<Box<Thread>>) -> R>(f: F) -> R {
    // Dispatcher-level critical section: local interrupts masked.
    without_interrupts(|| WAIT_DB.with_locked(f))
}

#[inline]
fn try_with_dispatch_db<R, F: FnOnce(&mut WaitDb<Box<Thread>>) -> R>(f: F) -> Option<R> {
    // Best-effort dispatcher-level lock acquisition.
    without_interrupts(|| WAIT_DB.try_with_locked(f))
}

impl WaitableThread for Box<Thread> {
    type Tid = ThreadId;

    #[inline]
    fn tid(&self) -> Self::Tid {
        self.id()
    }

    #[inline]
    fn mark_blocked(&mut self) {
        // Safety: called by scheduler during controlled block transition.
        unsafe { self.set_state(ThreadState::Blocked) };
    }

    #[inline]
    fn finish_wait(&mut self, wake_result: WakeResult) {
        self.disarm_wait();
        self.set_wake_result(wake_result);
    }
}
