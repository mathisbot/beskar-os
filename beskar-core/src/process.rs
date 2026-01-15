use crate::time::{Duration, Instant};
use core::sync::atomic::{AtomicU64, Ordering};

pub mod binary;

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
    pub const SLEEP_HANDLE_KEYBOARD_INTERRUPT: Self = Self(1);

    const SLEEP_HANDLE_FREE: u64 = 2;

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

/// The reason a thread is parked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepReason {
    /// Sleep until the given absolute deadline.
    Until(Instant),
    /// Sleep until a handle is signalled.
    Event(SleepHandle),
    /// Sleep without a deadline; the thread must be woken explicitly.
    Indefinite,
}

impl SleepReason {
    /// Builds a deadline-based reason from a duration and a reference instant.
    #[must_use]
    #[inline]
    pub fn for_duration(now: Instant, duration: Duration) -> Self {
        Self::Until(now + duration)
    }

    /// Returns the wake-up deadline if one exists.
    #[must_use]
    #[inline]
    pub const fn deadline(&self) -> Option<Instant> {
        match self {
            Self::Until(deadline) => Some(*deadline),
            _ => None,
        }
    }
}

pub struct AtomicSleepReason(AtomicU64);

impl AtomicSleepReason {
    const DISCRIMINANT_MASK: u64 = 0b11;

    const DISCRIMINANT_NONE: u64 = 0b00;
    const DISCRIMINANT_UNTIL: u64 = 0b01;
    const DISCRIMINANT_EVENT: u64 = 0b10;
    const DISCRIMINANT_INDEFINITE: u64 = 0b11;

    const DATA_SHIFT: u32 = 2;

    /// Packs a SleepReason into a raw u64.
    const fn pack(reason: Option<SleepReason>) -> u64 {
        match reason {
            None => Self::DISCRIMINANT_NONE,
            Some(SleepReason::Indefinite) => Self::DISCRIMINANT_INDEFINITE,
            Some(SleepReason::Event(handle)) => {
                let handle_raw = handle.raw();
                // Ensure the handle fits in the available bits
                // This limits us to 62 bits for the handle value, which is about
                // 4.6e18 unique handles (acceptable!).
                debug_assert!(handle_raw < (1 << 62), "SleepHandle raw value too large");
                (handle_raw << Self::DATA_SHIFT) | Self::DISCRIMINANT_EVENT
            }
            Some(SleepReason::Until(instant)) => {
                let micros_raw = instant.total_micros();
                // Mask out low bits to avoid colliding with discriminants
                // This limits precision to ~4 microseconds, which is acceptable
                (micros_raw & !Self::DISCRIMINANT_MASK) | Self::DISCRIMINANT_UNTIL
            }
        }
    }

    /// Unpacks a raw u64 into a SleepReason.
    ///
    /// # Safety
    ///
    /// The raw value must have been created by `pack`.
    const unsafe fn unpack(raw: u64) -> Option<SleepReason> {
        let discriminant = raw & Self::DISCRIMINANT_MASK;
        match discriminant {
            Self::DISCRIMINANT_INDEFINITE => Some(SleepReason::Indefinite),
            Self::DISCRIMINANT_EVENT => {
                let handle_raw = raw >> Self::DATA_SHIFT;
                Some(SleepReason::Event(SleepHandle::from_raw(handle_raw)))
            }
            Self::DISCRIMINANT_UNTIL => {
                let micros_raw = raw & !Self::DISCRIMINANT_MASK;
                Some(SleepReason::Until(Instant::from_micros(micros_raw)))
            }
            Self::DISCRIMINANT_NONE => None,
            _ => unsafe { core::hint::unreachable_unchecked() },
        }
    }

    #[must_use]
    pub const fn new(reason: Option<SleepReason>) -> Self {
        Self(AtomicU64::new(Self::pack(reason)))
    }

    #[must_use]
    pub fn load(&self, order: Ordering) -> Option<SleepReason> {
        let raw = self.0.load(order);
        unsafe { Self::unpack(raw) }
    }

    pub fn store(&self, reason: Option<SleepReason>, order: Ordering) {
        let raw = Self::pack(reason);
        self.0.store(raw, order);
    }

    #[must_use]
    pub fn swap(&self, reason: Option<SleepReason>, order: Ordering) -> Option<SleepReason> {
        let raw = Self::pack(reason);
        let old_raw = self.0.swap(raw, order);
        unsafe { Self::unpack(old_raw) }
    }
}
