use crate::{
    error::{SyscallError, SyscallResult},
    sys,
    time::Duration,
};
use beskar_core::process::{SleepHandle, WaitResult};

#[inline]
/// Spawns a new thread that starts executing at the given entry point.
///
/// # Errors
///
/// Returns an error if the syscall fails.
pub fn spawn(entry: extern "C" fn() -> !) -> SyscallResult<u64> {
    let res = sys::sc_thread_spawn(entry);
    if res == 0 {
        Err(SyscallError::new(-1))
    } else {
        Ok(res)
    }
}

#[inline]
/// Sleep for **at least** the given duration.
///
/// # Errors
///
/// Returns an error if the syscall fails.
pub fn sleep(duration: Duration) -> SyscallResult<()> {
    let code = sys::sc_wait_on_event(SleepHandle::NONE, duration.total_micros());
    match code {
        WaitResult::Timeout => Ok(()),
        _ => Err(SyscallError::new(-1)),
    }
}
