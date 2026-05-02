use crate::arch::syscalls;
use beskar_core::{
    process::{SleepHandle, WaitResult, sync::FutexWaitResult},
    syscall::{ExitCode, Syscall, SyscallExitCode},
    video::SurfaceId,
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
pub fn sc_wait_on_event(handle: SleepHandle, timeout_us: u64) -> WaitResult {
    let res = syscalls::syscall_2(Syscall::WaitOnEvent, handle.raw(), timeout_us);
    WaitResult::try_from(res).unwrap()
}

#[inline]
pub fn sc_futex_wait(value: *const u64, expected: u64, timeout_us: u64) -> FutexWaitResult {
    let res = syscalls::syscall_3(Syscall::FutexWait, value as u64, expected, timeout_us);
    FutexWaitResult::try_from(res).unwrap()
}

#[inline]
pub fn sc_futex_wake(value: *const u64, wake_count: usize) -> usize {
    let res = syscalls::syscall_2(
        Syscall::FutexWake,
        value as u64,
        u64::try_from(wake_count).unwrap_or(u64::MAX),
    );

    usize::try_from(res).unwrap_or(usize::MAX)
}

pub fn sc_surface_create(width: u16, height: u16, x: u16, y: u16, buffer: *const u8) -> i64 {
    let res = syscalls::syscall_3(
        Syscall::SurfaceCreate,
        (u64::from(width) << 16) | u64::from(height),
        (u64::from(x) << 16) | u64::from(y),
        buffer as u64,
    );
    res.cast_signed()
}

#[inline]
pub fn sc_surface_destroy(surface_id: SurfaceId) -> SyscallExitCode {
    let res = syscalls::syscall_1(Syscall::SurfaceDestroy, u64::from(surface_id.0));
    SyscallExitCode::try_from(res).unwrap()
}

#[inline]
pub fn sc_surface_dirty(
    surface_id: SurfaceId,
    width: u16,
    height: u16,
    x: u16,
    y: u16,
) -> SyscallExitCode {
    let res = syscalls::syscall_3(
        Syscall::SurfaceDirty,
        u64::from(surface_id.0),
        (u64::from(width) << 16) | u64::from(height),
        (u64::from(x) << 16) | u64::from(y),
    );
    SyscallExitCode::try_from(res).unwrap()
}

#[inline]
pub fn sc_surface_present(surface_id: SurfaceId) -> SyscallExitCode {
    let res = syscalls::syscall_1(Syscall::SurfacePresent, u64::from(surface_id.0));
    SyscallExitCode::try_from(res).unwrap()
}

#[inline]
pub fn sc_query_config(info_type: u64, buffer: *mut (), buffer_size: u64) -> SyscallExitCode {
    let res = syscalls::syscall_3(Syscall::QueryConfig, info_type, buffer as u64, buffer_size);
    SyscallExitCode::try_from(res).unwrap()
}

#[inline]
pub fn sc_thread_spawn(entry_point: extern "C" fn() -> !) -> u64 {
    let entry = entry_point as *const () as u64;
    syscalls::syscall_1(Syscall::ThreadSpawn, entry)
}
