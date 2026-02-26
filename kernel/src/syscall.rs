use crate::process;
use beskar_core::{
    arch::{
        VirtAddr,
        paging::{CacheFlush, M4KiB, Mapper, MappingError, MemSize, Page},
    },
    syscall::{Syscall, SyscallExitCode, SyscallReturnValue},
};
use beskar_hal::paging::page_table::Flags;

pub fn init() {
    crate::arch::syscall::init_syscalls();
}

#[derive(Debug, Copy, Clone)]
#[expect(dead_code, reason = "Some fields may not be used yet")]
pub struct Arguments {
    pub one: u64,
    pub two: u64,
    pub three: u64,
    pub four: u64,
    pub five: u64,
    pub six: u64,
}

/// Validate that a memory range is owned by the current process
/// and is located within its user-space address space.
#[must_use]
#[inline]
pub fn probe(start: VirtAddr, end: VirtAddr) -> bool {
    process::current().address_space().is_addr_owned(start, end)
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
        Syscall::Sleep => SyscallReturnValue::Code(sc_sleep(args)),
        Syscall::WaitOnEvent => SyscallReturnValue::Code(sc_wait_on_event(args)),
        Syscall::SurfaceCreate => SyscallReturnValue::ValueI(sc_surface_create(args)),
        Syscall::SurfaceDestroy => SyscallReturnValue::Code(sc_surface_destroy(args)),
        Syscall::SurfaceDirty => SyscallReturnValue::Code(sc_surface_dirty(args)),
        Syscall::SurfacePresent => SyscallReturnValue::Code(sc_surface_present(args)),
        Syscall::QueryConfig => SyscallReturnValue::Code(sc_query_config(args)),
    }
}

fn sc_exit(args: &Arguments) -> ! {
    let exit_code = args.one;

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
    let mut flags = Flags::USER_ACCESSIBLE;
    if raw & beskar_core::syscall::consts::MFLAGS_READ != 0 {
        flags |= Flags::PRESENT;
    }
    if raw & beskar_core::syscall::consts::MFLAGS_WRITE != 0 {
        flags |= Flags::WRITABLE;
    }
    if raw & beskar_core::syscall::consts::MFLAGS_EXECUTE == 0 {
        flags |= Flags::NO_EXECUTE;
    }
    flags
}

#[must_use]
fn sc_mmap(args: &Arguments) -> u64 {
    let len = args.one;
    if len == 0 {
        return 0;
    }
    let align = args.two;
    if !align.is_power_of_two() || align > M4KiB::SIZE {
        // TODO: Support larger alignments
        return 0;
    }
    let flags_raw = args.three;

    let flags = build_flags_from_us(flags_raw);

    let Some(page_range) = process::current()
        .address_space()
        .alloc_map::<M4KiB>(usize::try_from(len).unwrap(), flags)
    else {
        return 0;
    };

    page_range.start().start_address().as_u64()
}

fn sc_munmap(args: &Arguments) -> SyscallExitCode {
    let ptr = args.one;
    let size = args.two;

    if size == 0 {
        return SyscallExitCode::Success;
    }

    let Some(va) = VirtAddr::try_new(ptr) else {
        return SyscallExitCode::Failure;
    };
    let end = va + (size - 1);

    if !va.is_aligned(beskar_core::arch::Alignment::Align4K)
        && !size.is_multiple_of(M4KiB::SIZE)
        && !probe(va, end)
    {
        return SyscallExitCode::Failure;
    }

    let page_start = va.page::<M4KiB>();
    let page_end = end.page::<M4KiB>();

    let page_range = Page::range_inclusive(page_start, page_end);

    unsafe { process::current().address_space().unmap_free(page_range) };

    SyscallExitCode::Success
}

#[must_use]
fn sc_mprotect(args: &Arguments) -> SyscallExitCode {
    let ptr = args.one;
    let size = args.two;
    let flags_raw = args.three;

    if size == 0 {
        return SyscallExitCode::Success;
    }

    let Some(va) = VirtAddr::try_new(ptr) else {
        return SyscallExitCode::Failure;
    };
    let end = va + (size - 1);

    if !va.is_aligned(beskar_core::arch::Alignment::Align4K)
        && !size.is_multiple_of(M4KiB::SIZE)
        && !probe(va, end)
    {
        return SyscallExitCode::Failure;
    }

    let flags = build_flags_from_us(flags_raw);

    let page_start = va.page::<M4KiB>();
    let page_end = end.page::<M4KiB>();

    let page_range = Page::range_inclusive(page_start, page_end);

    let res =
        process::current()
            .address_space()
            .with_page_table(|pt| -> Result<_, MappingError<_>> {
                for page in page_range {
                    let cache_flush = pt.update_flags(page, flags)?;
                    cache_flush.flush();
                }
                Ok(())
            });

    match res {
        Ok(()) => SyscallExitCode::Success,
        Err(_) => SyscallExitCode::Failure,
    }
}

#[must_use]
fn sc_read(args: &Arguments) -> i64 {
    let file_handle = {
        let raw = args.one.cast_signed();
        if raw < 0 {
            return -1;
        }
        // Safety: The handle is used for comparison only
        // and the given value is positive.
        unsafe { ::storage::vfs::Handle::from_raw(raw) }
    };

    let buffer_start = VirtAddr::try_new(args.two).unwrap_or_default();
    let buffer_len = args.three;

    if !probe(buffer_start, buffer_start + buffer_len) {
        return -1;
    }

    // Safety: The buffer's range is owned by the curent process.
    let buffer = unsafe {
        core::slice::from_raw_parts_mut(buffer_start.as_mut_ptr(), buffer_len.try_into().unwrap())
    };

    let file_offset = usize::try_from(args.four).unwrap();

    let res = crate::storage::vfs().read(file_handle, buffer, file_offset);
    res.map_or(-1, |bytes_read| {
        i64::try_from(bytes_read).unwrap_or(i64::MAX)
    })
}

#[must_use]
fn sc_write(args: &Arguments) -> i64 {
    let file_handle = {
        let raw = args.one.cast_signed();
        if raw < 0 {
            return -1;
        }
        // Safety: The handle is used for comparison only
        // and the given value is positive.
        unsafe { ::storage::vfs::Handle::from_raw(raw) }
    };
    let buffer_start = VirtAddr::try_new(args.two).unwrap_or_default();
    let buffer_len = args.three;

    if !probe(buffer_start, buffer_start + buffer_len) {
        return -1;
    }

    // Safety: The buffer's range is owned by the curent process.
    let buffer = unsafe {
        core::slice::from_raw_parts(buffer_start.as_ptr(), buffer_len.try_into().unwrap())
    };

    let file_offset = usize::try_from(args.four).unwrap();

    let res = crate::storage::vfs().write(file_handle, buffer, file_offset);
    res.map_or(-1, |bytes_written| {
        i64::try_from(bytes_written).unwrap_or(i64::MAX)
    })
}

#[must_use]
fn sc_open(args: &Arguments) -> i64 {
    use ::storage::{fs::Path, vfs::Handle};

    let path_start = VirtAddr::try_new(args.one).unwrap_or_default();
    let path_len = args.two;

    if !probe(path_start, path_start + path_len) {
        return Handle::INVALID.id();
    }

    // Safety: The buffer's range is owned by the curent process.
    let raw_path =
        unsafe { core::slice::from_raw_parts(path_start.as_ptr(), path_len.try_into().unwrap()) };
    let Ok(path) = core::str::from_utf8(raw_path) else {
        return Handle::INVALID.id();
    };

    let res = crate::storage::vfs().open(Path::from(path));
    res.map_or(-1, |handle| handle.id())
}

#[must_use]
fn sc_close(args: &Arguments) -> SyscallExitCode {
    let file_handle = {
        let raw = args.one.cast_signed();
        if raw < 0 {
            return SyscallExitCode::Failure;
        }
        // Safety: The handle is used for comparison only
        // and the given value is positive.
        unsafe { ::storage::vfs::Handle::from_raw(raw) }
    };
    let res = crate::storage::vfs().close(file_handle);

    match res {
        Ok(()) => SyscallExitCode::Success,
        Err(_) => SyscallExitCode::Failure, // TODO: Differentiate between errors.
    }
}

#[must_use]
fn sc_sleep(args: &Arguments) -> SyscallExitCode {
    let sleep_time_ms = args.one;

    let sleep_time = crate::time::Duration::from_millis(sleep_time_ms);

    crate::process::scheduler::sleep_for(sleep_time);

    SyscallExitCode::Success
}

#[must_use]
fn sc_wait_on_event(args: &Arguments) -> SyscallExitCode {
    let handle_raw = args.one;
    let handle = beskar_core::process::SleepHandle::from_raw(handle_raw);

    crate::process::scheduler::sleep_on(handle);

    SyscallExitCode::Success
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Arguments are passed as u64 but represent smaller types"
)]
fn sc_surface_create(args: &Arguments) -> i64 {
    let width = (args.one >> 16) as u16;
    let height = args.one as u16;
    let x = (args.two >> 16) as u16;
    let y = args.two as u16;
    let user_buffer_ptr = args.three as *mut u8;

    if width == 0 || height == 0 {
        return -1;
    }

    let user_buffer = VirtAddr::from_ptr(user_buffer_ptr);
    let buffer_size = u64::from(width) * u64::from(height) * 4;
    let buffer_end = user_buffer + buffer_size;
    if !probe(user_buffer, buffer_end) {
        return -1;
    }

    let res = crate::video::with_compositor(|c| {
        let sid = unsafe { c.create_surface_with_buffer(x, y, width, height, user_buffer_ptr) };
        (sid, crate::video::SurfaceGuard(sid))
    });

    if let Some((raw_sid, guard)) = res {
        // Register the surface with the current process for automatic cleanup
        let process = crate::process::current();
        let registered = process.register_surface(guard);
        crate::trace::set_screen_logging(false);
        if registered { i64::from(raw_sid.0) } else { -1 }
    } else {
        -1
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Arguments are passed as u64 but represent smaller types"
)]
fn sc_surface_destroy(args: &Arguments) -> SyscallExitCode {
    let sid_raw = args.one as u32;
    let sid = beskar_core::video::SurfaceId(sid_raw);

    crate::video::with_compositor(|c| c.destroy_surface(sid));

    SyscallExitCode::Success
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Arguments are passed as u64 but represent smaller types"
)]
fn sc_surface_dirty(args: &Arguments) -> SyscallExitCode {
    let sid_raw = args.one as u32;
    let sid = beskar_core::video::SurfaceId(sid_raw);
    let width = (args.two >> 16) as u16;
    let height = args.two as u16;
    let x = (args.three >> 16) as u16;
    let y = args.three as u16;

    let rect = beskar_core::video::Rect::new(x, y, width, height);

    // Render only this surface synchronously in the syscall context
    // where we can safely access the userspace buffer
    let result = crate::video::with_compositor(|c| c.mark_surface_dirty(sid, rect).ok()).flatten();

    if result.is_some() {
        SyscallExitCode::Success
    } else {
        SyscallExitCode::Failure
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Arguments are passed as u64 but represent smaller types"
)]
fn sc_surface_present(args: &Arguments) -> SyscallExitCode {
    let sid_raw = args.one as u32;

    // Render only this surface synchronously in the syscall context
    // where we can safely access the userspace buffer
    let result = crate::video::with_compositor(|c| {
        c.render_surface_dirty(beskar_core::video::SurfaceId(sid_raw))
            .ok()
    })
    .flatten();

    if result.is_some() {
        SyscallExitCode::Success
    } else {
        SyscallExitCode::Failure
    }
}

fn sc_query_config(args: &Arguments) -> SyscallExitCode {
    let query_type = args.one;
    let output_ptr = args.two as *mut ();
    let output_size = args.three;

    let start = VirtAddr::from_ptr(output_ptr);
    let end = start + output_size;
    if !probe(start, end) {
        return SyscallExitCode::Failure;
    }

    let size = usize::try_from(output_size).unwrap_or(0);
    match query_type {
        beskar_core::syscall::consts::QUERY_FRAMEBUFFER => {
            if size < core::mem::size_of::<beskar_core::video::Info>() {
                return SyscallExitCode::Failure;
            }
            let Some(info) = crate::video::with_compositor(|c| c.config().info()) else {
                return SyscallExitCode::Failure;
            };
            unsafe {
                output_ptr
                    .cast::<beskar_core::video::Info>()
                    .write_unaligned(info);
            }
            SyscallExitCode::Success
        }
        _ => SyscallExitCode::Failure,
    }
}
