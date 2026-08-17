use crate::arch::syscall::Arguments;
use crate::mem::vmm;
use beskar_core::{
    arch::{
        Alignment, VirtAddr,
        paging::{M4KiB, MemSize, Page},
    },
    syscall::{Syscall, SyscallExitCode, SyscallReturnValue},
};
use beskar_hal::paging::page_table::Flags;

pub fn init() {
    crate::arch::syscall::init_syscalls();
}

/// Validate that a memory range is owned by the current process
/// and is located within its user-space address space.
#[must_use]
#[inline]
pub fn is_addr_owned(start: VirtAddr, end: VirtAddr) -> bool {
    vmm::process_local::is_addr_owned(start, end)
}

#[must_use]
pub fn syscall(syscall: Syscall, args: &Arguments) -> SyscallReturnValue {
    match syscall {
        Syscall::Exit => sc_exit(args),
        Syscall::MemoryMap => SyscallReturnValue::ValueU(sc_mmap(args)),
        Syscall::MemoryUnmap => SyscallReturnValue::Code(sc_munmap(args)),
        Syscall::MemoryProtect => SyscallReturnValue::Code(sc_mprotect(args)),
        Syscall::Read => SyscallReturnValue::ValueI(sc_read(args)),
        Syscall::Write => SyscallReturnValue::ValueI(sc_write(args)),
        Syscall::Open => SyscallReturnValue::ValueI(sc_open(args)),
        Syscall::Close => SyscallReturnValue::Code(sc_close(args)),
        Syscall::WaitOnEvent => SyscallReturnValue::ValueU(sc_wait_on_event(args)),
        Syscall::FutexWait => SyscallReturnValue::ValueU(sc_futex_wait(args)),
        Syscall::FutexWake => SyscallReturnValue::ValueU(sc_futex_wake(args)),
        Syscall::SurfaceCreate => SyscallReturnValue::Code(sc_surface_create(args)),
        Syscall::SurfaceDestroy => SyscallReturnValue::Code(sc_surface_destroy(args)),
        Syscall::SurfaceDirty => SyscallReturnValue::Code(sc_surface_dirty(args)),
        Syscall::SurfacePresent => SyscallReturnValue::Code(sc_surface_present(args)),
        Syscall::QueryConfig => SyscallReturnValue::Code(sc_query_config(args)),
        Syscall::ThreadSpawn => SyscallReturnValue::ValueU(sc_thread_spawn(args)),
        Syscall::PowerManagement => SyscallReturnValue::Code(sc_powermgt(args)),
        Syscall::Ping => SyscallReturnValue::ValueI(sc_ping(args)),
        Syscall::PrecisionTimer => SyscallReturnValue::ValueU(sc_precision_timer(args)),
    }
}

fn sc_exit(args: &Arguments) -> ! {
    let exit_code = args.one();

    if cfg!(debug_assertions) {
        let exit_code = beskar_core::syscall::ExitCode::try_from(exit_code);
        let tid = crate::process::scheduler::current_thread_id();

        if let Ok(exit_code) = exit_code {
            crate::debug!("Thread {} exited with code {:?}", tid.as_u64(), exit_code);
        } else {
            crate::debug!("Thread {} exited with invalid code", tid.as_u64());
        }
    }

    unsafe { crate::process::scheduler::exit_current_thread() }
}

#[must_use]
/// Build page table flags from user-space protection flags constants.
fn build_flags_from_us(raw: u64) -> Flags {
    // let readable = raw & beskar_core::syscall::consts::MFLAGS_READ != 0;
    let writable = raw & beskar_core::syscall::consts::MFLAGS_WRITE != 0;
    let executable = raw & beskar_core::syscall::consts::MFLAGS_EXECUTE != 0;

    let mut flags = Flags::USER_ACCESSIBLE | Flags::PRESENT;
    if writable {
        flags |= Flags::WRITABLE;
    }
    if !executable {
        flags |= Flags::NO_EXECUTE;
    }
    flags
}

#[must_use]
fn sc_mmap(args: &Arguments) -> u64 {
    let len = args.one();
    if len == 0 {
        return 0;
    }
    let align = args.two();
    if !align.is_power_of_two() || align > M4KiB::SIZE {
        // TODO: Support larger alignments
        return 0;
    }
    let flags_raw = args.three();

    let flags = build_flags_from_us(flags_raw);

    let Some(page_range) =
        vmm::process_local::alloc_map::<M4KiB>(usize::try_from(len).unwrap(), flags)
    else {
        return 0;
    };

    page_range.start().start_address().as_u64()
}

fn sc_munmap(args: &Arguments) -> SyscallExitCode {
    let ptr = args.one();
    let size = args.two();

    if size == 0 {
        return SyscallExitCode::Success;
    }

    let Some(va) = VirtAddr::try_new(ptr) else {
        return SyscallExitCode::Failure;
    };
    let end = va + (size - 1);

    if !va.is_aligned(beskar_core::arch::Alignment::Align4K)
        || !size.is_multiple_of(M4KiB::SIZE)
        || !is_addr_owned(va, end)
    {
        return SyscallExitCode::Failure;
    }

    let page_start = va.page::<M4KiB>();
    let page_end = end.page::<M4KiB>();

    let page_range = Page::range_inclusive(page_start, page_end);

    unsafe { vmm::process_local::unmap_free(page_range) };

    SyscallExitCode::Success
}

#[must_use]
fn sc_mprotect(args: &Arguments) -> SyscallExitCode {
    let ptr = args.one();
    let size = args.two();
    let flags_raw = args.three();

    if size == 0 {
        return SyscallExitCode::Success;
    }

    let Some(va) = VirtAddr::try_new(ptr) else {
        return SyscallExitCode::Failure;
    };
    let end = va + (size - 1);

    if !va.is_aligned(beskar_core::arch::Alignment::Align4K)
        || !size.is_multiple_of(M4KiB::SIZE)
        || !is_addr_owned(va, end)
    {
        return SyscallExitCode::Failure;
    }

    let flags = build_flags_from_us(flags_raw);

    let page_start = va.page::<M4KiB>();
    let page_end = end.page::<M4KiB>();

    let page_range = Page::range_inclusive(page_start, page_end);

    let res = vmm::process_local::update_flags(page_range, flags);

    match res {
        Ok(()) => SyscallExitCode::Success,
        Err(_) => SyscallExitCode::Failure,
    }
}

#[must_use]
fn sc_read(args: &Arguments) -> i64 {
    let file_handle = {
        let raw = args.one().cast_signed();
        if raw < 0 {
            return -1;
        }
        // Safety: The handle is used for comparison only
        // and the given value is positive.
        unsafe { ::storage::vfs::Handle::from_raw(raw) }
    };

    let buffer_start = VirtAddr::try_new(args.two()).unwrap_or_default();
    let buffer_len = args.three();

    if !is_addr_owned(buffer_start, buffer_start + buffer_len) {
        return -1;
    }
    if !crate::process::current().perms().fs_read() {
        return -1;
    }

    // Safety: The buffer's range is owned by the curent process.
    let buffer = unsafe {
        core::slice::from_raw_parts_mut(buffer_start.as_mut_ptr(), buffer_len.try_into().unwrap())
    };

    let file_offset = usize::try_from(args.four()).unwrap();

    let pid = crate::process::scheduler::current_process().pid();
    let res = crate::storage::vfs().read(pid.as_u64(), file_handle, buffer, file_offset);
    res.map_or(-1, |bytes_read| {
        i64::try_from(bytes_read).unwrap_or(i64::MAX)
    })
}

#[must_use]
fn sc_write(args: &Arguments) -> i64 {
    let file_handle = {
        let raw = args.one().cast_signed();
        if raw < 0 {
            return -1;
        }
        // Safety: The handle is used for comparison only
        // and the given value is positive.
        unsafe { ::storage::vfs::Handle::from_raw(raw) }
    };
    let buffer_start = VirtAddr::try_new(args.two()).unwrap_or_default();
    let buffer_len = args.three();

    if !is_addr_owned(buffer_start, buffer_start + buffer_len) {
        return -1;
    }
    if !crate::process::current().perms().fs_write() {
        return -1;
    }

    // Safety: The buffer's range is owned by the curent process.
    let buffer = unsafe {
        core::slice::from_raw_parts(buffer_start.as_ptr(), buffer_len.try_into().unwrap())
    };

    let file_offset = usize::try_from(args.four()).unwrap();

    let pid = crate::process::scheduler::current_process().pid();
    let res = crate::storage::vfs().write(pid.as_u64(), file_handle, buffer, file_offset);
    res.map_or(-1, |bytes_written| {
        i64::try_from(bytes_written).unwrap_or(i64::MAX)
    })
}

#[must_use]
fn sc_open(args: &Arguments) -> i64 {
    use ::storage::{fs::Path, vfs::Handle};

    let path_start = VirtAddr::try_new(args.one()).unwrap_or_default();
    let path_len = args.two();

    if !is_addr_owned(path_start, path_start + path_len) {
        return Handle::INVALID.id();
    }

    // Safety: The buffer's range is owned by the curent process.
    let raw_path =
        unsafe { core::slice::from_raw_parts(path_start.as_ptr(), path_len.try_into().unwrap()) };
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return Handle::INVALID.id();
    };

    let pid = crate::process::scheduler::current_process().pid();
    let res = crate::storage::vfs().open(pid.as_u64(), Path::from(path));
    res.map_or(-1, |handle| handle.id())
}

#[must_use]
fn sc_close(args: &Arguments) -> SyscallExitCode {
    let file_handle = {
        let raw = args.one().cast_signed();
        if raw < 0 {
            return SyscallExitCode::Failure;
        }
        // Safety: The handle is used for comparison only
        // and the given value is positive.
        unsafe { ::storage::vfs::Handle::from_raw(raw) }
    };

    let pid = crate::process::scheduler::current_process().pid();
    let res = crate::storage::vfs().close(pid.as_u64(), file_handle);

    match res {
        Ok(()) => SyscallExitCode::Success,
        Err(_) => SyscallExitCode::Failure, // TODO: Differentiate between errors.
    }
}

#[must_use]
fn sc_wait_on_event(args: &Arguments) -> u64 {
    let handle_raw = args.one();
    let timeout_us_raw = args.two();

    let handle = core::num::NonZeroU64::new(handle_raw)
        .map(|h| beskar_core::process::SleepHandle::from_raw(h.get()));
    let timeout_us = core::num::NonZeroU64::new(timeout_us_raw);

    if handle.is_none() && timeout_us.is_none() {
        return u64::from(beskar_core::process::WaitResult::Unknown);
    }

    let deadline = timeout_us
        .map(|us| crate::time::Instant::now() + crate::time::Duration::from_micros(us.get()));
    let deadline = deadline.map(crate::time::Instant::as_inner);
    let wake = crate::process::scheduler::wait(wait::WaitRequest::new(handle, deadline));

    u64::from(beskar_core::process::WaitResult::from(wake.cause()))
}

#[must_use]
fn sc_futex_wait(args: &Arguments) -> u64 {
    use beskar_core::process::sync::FutexWaitResult;
    use core::sync::atomic::AtomicU64;

    let ptr = args.one();
    let size = size_of::<u64>() as u64;
    let expected = args.two();
    let timeout_us = args.three();

    let Some(futex_addr) = VirtAddr::try_new(ptr) else {
        return u64::from(FutexWaitResult::InvalidAddress);
    };
    let futex_end = futex_addr + (size - 1);
    if !futex_addr.is_aligned(Alignment::of::<u64>()) || !is_addr_owned(futex_addr, futex_end) {
        return u64::from(FutexWaitResult::InvalidAddress);
    }

    // Safety: the pointer was validated as user-owned and 8-byte aligned above.
    let futex_word = unsafe { AtomicU64::from_ptr(futex_addr.as_mut_ptr()) };
    let wait = if timeout_us == 0 {
        crate::process::sync::Futex::wait_on_address(futex_word, expected)
    } else {
        let timeout = crate::time::Duration::from_micros(timeout_us);
        crate::process::sync::Futex::wait_on_address_for(futex_word, expected, timeout)
    };

    u64::from(wait)
}

#[must_use]
fn sc_futex_wake(args: &Arguments) -> u64 {
    use core::sync::atomic::AtomicU64;

    let ptr = args.one();
    let size = size_of::<u64>() as u64;
    let amount = args.two();

    let Some(futex_addr) = VirtAddr::try_new(ptr) else {
        return 0;
    };
    let futex_end = futex_addr + (size - 1);
    if !futex_addr.is_aligned(Alignment::of::<u64>()) || !is_addr_owned(futex_addr, futex_end) {
        return 0;
    }

    let wake_count = usize::try_from(amount).unwrap_or(usize::MAX);
    if wake_count == 0 {
        return 0;
    }

    // Safety: the pointer was validated as user-owned and 8-byte aligned above.
    let futex_word = unsafe { AtomicU64::from_ptr(futex_addr.as_mut_ptr()) };

    let woken = if wake_count == usize::MAX {
        crate::process::sync::Futex::wake_by_address_all(futex_word)
    } else {
        crate::process::sync::Futex::wake_by_address_n(futex_word, wake_count)
    };

    u64::try_from(woken).unwrap_or(u64::MAX)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Arguments are passed as u64 but represent smaller types"
)]
fn sc_surface_create(args: &Arguments) -> SyscallExitCode {
    let width = (args.one() >> 16) as u16;
    let height = args.one() as u16;
    let x = (args.two() >> 16) as u16;
    let y = args.two() as u16;
    let user_buffer_ptr = args.three() as *mut u8;

    if width == 0 || height == 0 {
        return SyscallExitCode::Failure;
    }

    let user_buffer = VirtAddr::from_ptr(user_buffer_ptr);
    let buffer_size = u64::from(width) * u64::from(height) * 4;
    let buffer_end = user_buffer + buffer_size;
    if !is_addr_owned(user_buffer, buffer_end) {
        return SyscallExitCode::Failure;
    }
    if !crate::process::current().perms().create_surface() {
        return SyscallExitCode::Failure;
    }

    let res = crate::video::with_compositor(|c| unsafe {
        c.create_surface_with_buffer(x, y, width, height, user_buffer_ptr)
    });

    if let Some(sid) = res {
        // Register the surface with the current process for automatic cleanup
        let process = crate::process::current();
        let registered = process.register_surface(sid);

        if registered.is_ok() {
            crate::trace::set_screen_logging(false);
            return SyscallExitCode::Success;
        }

        let res = crate::video::with_compositor(|c| c.destroy_surface(sid));
        debug_assert!(res.is_some());
    }

    SyscallExitCode::Failure
}

#[expect(clippy::option_if_let_else, reason = "Readability")]
fn sc_surface_destroy(_args: &Arguments) -> SyscallExitCode {
    let res = crate::process::current().destroy_surface();
    if let Some(sg) = res {
        drop(sg);
        SyscallExitCode::Success
    } else {
        SyscallExitCode::Failure
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Arguments are passed as u64 but represent smaller types"
)]
fn sc_surface_dirty(args: &Arguments) -> SyscallExitCode {
    let width = (args.one() >> 16) as u16;
    let height = args.one() as u16;
    let x = (args.two() >> 16) as u16;
    let y = args.two() as u16;

    let rect = beskar_core::video::Rect::new(x, y, width, height);

    let Some(sid) = crate::process::current().surface() else {
        return SyscallExitCode::Failure;
    };

    let result = crate::video::with_compositor(|c| c.mark_surface_dirty(sid, rect).ok()).flatten();

    if result.is_some() {
        SyscallExitCode::Success
    } else {
        SyscallExitCode::Failure
    }
}

fn sc_surface_present(args: &Arguments) -> SyscallExitCode {
    let present_all = (args.one() & 0b1) != 0;

    let Some(sid) = crate::process::current().surface() else {
        return SyscallExitCode::Failure;
    };

    let result = crate::video::with_compositor(|c| {
        if present_all {
            c.mark_surface_all_dirty(sid).ok()?;
        }
        c.render_surface_dirty(sid).ok()
    })
    .flatten();

    if result.is_some() {
        SyscallExitCode::Success
    } else {
        SyscallExitCode::Failure
    }
}

fn sc_query_config(args: &Arguments) -> SyscallExitCode {
    fn fill<T, F: FnOnce() -> Option<T>>(ptr: *mut (), size: usize, f: F) -> SyscallExitCode {
        if size >= size_of::<T>()
            && let Some(v) = f()
        {
            unsafe { ptr.cast::<T>().write_unaligned(v) };
            SyscallExitCode::Success
        } else {
            SyscallExitCode::Failure
        }
    }

    let query_type = args.one();
    let output_ptr = args.two() as *mut ();
    let output_size = args.three();

    let start = VirtAddr::from_ptr(output_ptr);
    let end = start + output_size;
    if !is_addr_owned(start, end) {
        return SyscallExitCode::Failure;
    }

    let size = usize::try_from(output_size).unwrap_or(0);
    match query_type {
        beskar_core::syscall::consts::QUERY_FRAMEBUFFER => fill(output_ptr, size, || {
            crate::video::with_compositor(|c| c.config().info())
        }),
        beskar_core::syscall::consts::QUERY_KEYBOARD_WAIT_HANDLE => {
            fill(output_ptr, size, crate::drivers::keyboard::wait_handle)
        }
        beskar_core::syscall::consts::QUERY_HIGH_PRES_TIMER => {
            fill(output_ptr, size, || crate::time::timer_info().copied())
        }

        _ => SyscallExitCode::Failure,
    }
}

fn sc_thread_spawn(args: &Arguments) -> u64 {
    use crate::process::scheduler::{self, thread};
    use beskar_core::process::ThreadStartBlock;

    let entry_point = args.one();

    let entry_point = VirtAddr::try_new(entry_point).unwrap_or_default();
    // Not technically necessary
    if !is_addr_owned(entry_point, entry_point) {
        return 0;
    }

    // SAFETY: This is effectively a transmute of a function pointer to a usize
    let start_fn = unsafe {
        core::mem::transmute::<
            extern "C" fn(extern "C" fn(*const ThreadStartBlock)) -> !,
            extern "C" fn(usize) -> !,
        >(thread::start_user_thread)
    };
    let thread = thread::Thread::builder_with_arg(
        scheduler::current_process(),
        start_fn,
        entry_point.as_u64().try_into().unwrap(),
    )
    .stack_heap(alloc::vec![0; 16 * 1024])
    .priority(scheduler::Priority::Normal)
    .build_boxed();

    let tid = thread.id().as_u64();
    debug_assert_ne!(tid, 0);

    scheduler::spawn_thread(thread);

    tid
}

fn sc_powermgt(args: &Arguments) -> SyscallExitCode {
    use beskar_core::syscall::consts;

    let action = args.one();

    if !crate::process::current().perms().power_mgmt() {
        return SyscallExitCode::Failure;
    }

    match action {
        consts::POWERMGT_SHUTDOWN => {
            unsafe { crate::power::shutdown() };
        }
        consts::POWERMGT_REBOOT => {
            unsafe { crate::power::reboot() };
        }
        _ => SyscallExitCode::Failure,
    }
}

fn sc_precision_timer(_args: &Arguments) -> u64 {
    crate::time::now_raw()
}

fn sc_ping(args: &Arguments) -> i64 {
    use beskar_core::syscall::consts;
    use holonet::{NetworkError, l3::ip::v4::Ipv4Addr};

    let Ok(raw_addr) = u32::try_from(args.one()) else {
        return consts::PING_INVALID;
    };
    let addr = Ipv4Addr::from_bits(raw_addr);

    let timeout = match args.two() {
        0 => crate::network::DEFAULT_ECHO_TIMEOUT,
        millis => crate::time::Duration::from_millis(millis),
    };

    match crate::network::ping(addr, timeout) {
        // The round trip cannot realistically overflow an i64 of microseconds,
        // but a saturating conversion keeps a bogus clock from reporting an
        // error code.
        Ok(outcome) => i64::try_from(outcome.round_trip.as_micros()).unwrap_or(i64::MAX),
        Err(NetworkError::Absent | NetworkError::Uninitialized) => consts::PING_NO_INTERFACE,
        Err(NetworkError::Unreachable) => consts::PING_UNREACHABLE,
        Err(_) => consts::PING_INVALID,
    }
}
