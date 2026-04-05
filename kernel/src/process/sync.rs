use alloc::collections::BTreeMap;
use beskar_core::{
    process::{SleepHandle, sync::FutexWaitResult},
    time::{Duration, Instant},
};
use core::sync::atomic::{AtomicU64, Ordering};
use hyperdrive::locks::mcs::MUMcsLock;

static FUTEX_TABLE: MUMcsLock<BTreeMap<FutexKey, FutexEntry>> = MUMcsLock::uninit();

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct FutexKey {
    pid: u64,
    addr: usize,
}

impl FutexKey {
    #[must_use]
    #[inline]
    fn from_atomic(value: &AtomicU64) -> Self {
        Self {
            pid: crate::process::current().pid().as_u64(),
            addr: core::ptr::from_ref(value).addr(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct FutexEntry {
    handle: SleepHandle,
    waiters: usize,
}

impl FutexEntry {
    #[must_use]
    #[inline]
    fn new() -> Self {
        Self {
            handle: SleepHandle::new(),
            waiters: 0,
        }
    }
}

pub struct Futex;

impl Futex {
    #[must_use]
    #[inline]
    /// Wait on an address while its value is equal to `expected`.
    pub fn wait_on_address(value: &AtomicU64, expected: u64) -> FutexWaitResult {
        Self::wait_on_address_until(value, expected, None)
    }

    #[must_use]
    #[inline]
    /// Wait on an address while its value is equal to `expected` until the given timeout elapses.
    pub fn wait_on_address_for(
        value: &AtomicU64,
        expected: u64,
        timeout: Duration,
    ) -> FutexWaitResult {
        let deadline = crate::time::now() + timeout;
        Self::wait_on_address_until(value, expected, Some(deadline))
    }

    #[must_use]
    /// Wait on an address while its value is equal to `expected` until an absolute deadline.
    pub fn wait_on_address_until(
        value: &AtomicU64,
        expected: u64,
        deadline: impl Into<Option<Instant>>,
    ) -> FutexWaitResult {
        if value.load(Ordering::Acquire) != expected {
            return FutexWaitResult::ValueMismatch;
        }

        let key = FutexKey::from_atomic(value);
        let handle = register_waiter(key);

        let wake = crate::process::scheduler::wait_if(
            ::wait::WaitRequest::new(Some(handle), deadline.into()),
            || value.load(Ordering::Acquire) == expected,
        );

        unregister_waiter(key);
        map_wait_result(value, expected, wake)
    }

    #[must_use]
    #[inline]
    /// Wake one waiter sleeping on this address.
    pub fn wake_by_address_single(value: &AtomicU64) -> usize {
        let Some(handle) = get_wait_handle(FutexKey::from_atomic(value)) else {
            return 0;
        };

        usize::from(crate::process::scheduler::wake_event_single(handle))
    }

    #[must_use]
    #[inline]
    /// Wake all waiters sleeping on this address.
    pub fn wake_by_address_all(value: &AtomicU64) -> usize {
        let Some(handle) = get_wait_handle(FutexKey::from_atomic(value)) else {
            return 0;
        };

        crate::process::scheduler::wake_event_all(handle)
    }

    #[must_use]
    /// Wake up to `count` waiters sleeping on this address.
    pub fn wake_by_address_n(value: &AtomicU64, count: usize) -> usize {
        if count == 0 {
            return 0;
        }

        let Some(handle) = get_wait_handle(FutexKey::from_atomic(value)) else {
            return 0;
        };

        if count == usize::MAX {
            return crate::process::scheduler::wake_event_all(handle);
        }

        let mut woken = 0;
        for _ in 0..count {
            if crate::process::scheduler::wake_event_single(handle) {
                woken += 1;
            } else {
                break;
            }
        }

        woken
    }
}

#[inline]
fn with_futex_table<R, F: FnOnce(&mut BTreeMap<FutexKey, FutexEntry>) -> R>(f: F) -> R {
    FUTEX_TABLE.init(BTreeMap::new());
    FUTEX_TABLE.with_locked(f)
}

#[must_use]
fn register_waiter(key: FutexKey) -> SleepHandle {
    with_futex_table(|table| {
        let entry = table.entry(key).or_insert_with(FutexEntry::new);
        entry.waiters = entry.waiters.saturating_add(1);
        entry.handle
    })
}

fn unregister_waiter(key: FutexKey) {
    with_futex_table(|table| {
        let should_remove = if let Some(entry) = table.get_mut(&key) {
            entry.waiters = entry.waiters.saturating_sub(1);
            entry.waiters == 0
        } else {
            false
        };

        if should_remove {
            table.remove(&key);
        }
    });
}

#[must_use]
fn get_wait_handle(key: FutexKey) -> Option<SleepHandle> {
    with_futex_table(|table| {
        table
            .get(&key)
            .and_then(|entry| (entry.waiters != 0).then_some(entry.handle))
    })
}

#[must_use]
fn map_wait_result(value: &AtomicU64, expected: u64, wake: ::wait::WakeResult) -> FutexWaitResult {
    match wake.cause() {
        ::wait::WakeCause::Event => FutexWaitResult::Woken,
        ::wait::WakeCause::Timeout => FutexWaitResult::TimedOut,
        ::wait::WakeCause::Killed => FutexWaitResult::Killed,
        ::wait::WakeCause::Cancelled if value.load(Ordering::Acquire) != expected => {
            FutexWaitResult::ValueMismatch
        }
        ::wait::WakeCause::Cancelled | ::wait::WakeCause::None => FutexWaitResult::Cancelled,
    }
}
