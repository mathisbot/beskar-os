use crate::arch::syscalls;
pub use beskar_core::drivers::keyboard::{KeyCode, KeyEvent, KeyState};
use beskar_core::syscall::{Syscall, SyscallExitCode};

#[inline]
/// Print a message to the console
///
/// ## Panics
///
/// Panics if the syscall fails (should never happen
/// for valid input).
pub fn print(msg: &str) {
    let res = syscalls::syscall_2(
        Syscall::Print,
        msg.as_ptr() as u64,
        msg.len().try_into().unwrap(),
    );
    assert_eq!(
        SyscallExitCode::try_from(res).unwrap_or(SyscallExitCode::Other),
        SyscallExitCode::Success
    );
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        $crate::io::print(&::alloc::format!($($arg)*));
    };
}

#[must_use]
#[inline]
/// Poll the kernel to get keyboard events
pub fn poll_keyboard() -> Option<KeyEvent> {
    let res = syscalls::syscall_0(Syscall::KeyboardPoll);
    unsafe { KeyEvent::unpack_option(res) }
}
