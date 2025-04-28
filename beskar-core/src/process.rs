use crate::arch::x86_64::userspace::Ring;
use core::sync::atomic::{AtomicU64, Ordering};

pub mod binary;

static PID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProcessId(u64);

impl core::ops::Deref for ProcessId {
    type Target = u64;

    fn deref(&self) -> &u64 {
        &self.0
    }
}

impl Default for ProcessId {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessId {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self(PID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    #[must_use]
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
pub enum Kind {
    /// Vital process kind.
    /// On panic, the system will be halted.
    Kernel,
    /// Driver process kind.
    /// These are Ring 0 processes that are not vital for the system.
    Driver,
    /// User process kind.
    /// These are Ring 3 processes.
    User,
}

impl Kind {
    #[must_use]
    #[inline]
    pub const fn ring(&self) -> Ring {
        match self {
            Self::Kernel | Self::Driver => Ring::Kernel,
            Self::User => Ring::User,
        }
    }
}
