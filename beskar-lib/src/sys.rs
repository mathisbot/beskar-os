use crate::arch::syscalls;
use beskar_core::{
    process::SleepHandle,
    syscall::{ExitCode, Syscall, SyscallExitCode},
};

#[inline]
pub fn sc_exit(code: ExitCode) -> ! {
    syscalls::syscall_1(Syscall::Exit, u64::from(code));
    unsafe { core::hint::unreachable_unchecked() }
}

#[inline]
pub fn sc_open(path: *const u8, len: u64) -> i64 {
    let res = syscalls::syscall_2(Syscall::Open, path as u64, len);
    res.cast_signed()
}

#[inline]
pub fn sc_close(handle: i64) -> SyscallExitCode {
    let res = syscalls::syscall_1(Syscall::Close, handle.cast_unsigned());
    SyscallExitCode::try_from(res).unwrap()
}

#[inline]
pub fn sc_read(handle: i64, buffer: *mut u8, size: u64, offset: u64) -> i64 {
    let res = syscalls::syscall_4(
        Syscall::Read,
        handle.cast_unsigned(),
        buffer as u64,
        size,
        offset,
    );
    res.cast_signed()
}

#[inline]
pub fn sc_write(handle: i64, buffer: *const u8, size: u64, offset: u64) -> i64 {
    let res = syscalls::syscall_4(
        Syscall::Write,
        handle.cast_unsigned(),
        buffer as u64,
        size,
        offset,
    );
    res.cast_signed()
}

#[inline]
pub fn sc_mmap(size: u64, alignment: u64, flags: u64) -> *mut u8 {
    let res = syscalls::syscall_3(Syscall::MemoryMap, size, alignment, flags);
    res as _
}

#[inline]
pub fn sc_munmap(ptr: *mut u8, size: u64) -> SyscallExitCode {
    let res = syscalls::syscall_2(Syscall::MemoryUnmap, ptr as u64, size);
    SyscallExitCode::try_from(res).unwrap()
}

#[inline]
pub fn sc_mprotect(ptr: *mut u8, size: u64, flags: u64) -> SyscallExitCode {
    let res = syscalls::syscall_3(Syscall::MemoryProtect, ptr as u64, size, flags);
    SyscallExitCode::try_from(res).unwrap()
}

#[inline]
pub fn sc_sleep(ms: u64) -> SyscallExitCode {
    let res = syscalls::syscall_1(Syscall::Sleep, ms);
    SyscallExitCode::try_from(res).unwrap()
}

#[inline]
pub fn sc_wait_on_event(handle: SleepHandle) -> SyscallExitCode {
    let res = syscalls::syscall_1(Syscall::WaitOnEvent, handle.raw());
    SyscallExitCode::try_from(res).unwrap()
}
