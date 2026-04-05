use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
pub enum FutexWaitResult {
    /// Wait was satisfied by a wake operation.
    Woken = 0,
    /// The futex value did not match the expected value at wait time.
    ValueMismatch = 1,
    /// Wait timed out.
    TimedOut = 2,
    /// Wait was cancelled.
    Cancelled = 3,
    /// Waiting thread was forcefully interrupted.
    Killed = 4,
    /// Pointer was invalid or not accessible.
    InvalidAddress = 5,
}
