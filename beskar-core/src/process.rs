use crate::arch::VirtAddr;
use core::sync::atomic::{AtomicU64, Ordering};
use num_enum::{IntoPrimitive, TryFromPrimitive};

pub mod binary;
pub mod perms;
pub mod sync;

/// Startup block passed to every userspace entry point in RDI.
///
/// This is a stable C ABI contract between the kernel and userspace runtime.
#[repr(C)]
#[derive(Debug, Default)]
pub struct ThreadStartBlock {
    /// The base address of the loaded binary in memory.
    pub start: VirtAddr,
}

/// A token that identifies a sleepable event.
///
/// Drivers and subsystems can hand these out so that threads can park until
/// the corresponding event is signalled (for example, an input device
/// interrupt). Tokens are cheap to create and are globally unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SleepHandle(u64);

impl Default for SleepHandle {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SleepHandle {
    pub const NONE: Self = Self(0);
    const SLEEP_HANDLE_FREE: u64 = 1;

    /// Creates a fresh handle that can later be signalled to wake sleepers.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(SleepHandle::SLEEP_HANDLE_FREE);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the raw numeric value of this handle.
    #[must_use]
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Reconstructs a handle from a raw value.
    #[must_use]
    #[inline]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
pub enum WaitResult {
    /// The wait was satisfied by a signal/event.
    Event = 0,
    /// The wait completed because timeout elapsed.
    Timeout = 1,
    /// The wait was cancelled.
    Cancelled = 2,
    /// The waiting thread was forcefully interrupted.
    Killed = 3,
    /// Unexpected wakeup source.
    Unknown = 4,
}
