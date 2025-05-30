use crate::process;
use beskar_core::{
    arch::{
        VirtAddr,
        paging::{CacheFlush as _, M4KiB, Mapper as _, MemSize},
    },
    syscall::{Syscall, SyscallExitCode, SyscallReturnValue},
};
use beskar_hal::paging::page_table::Flags;

pub fn init() {
    crate::arch::syscall::init_syscalls();
}

#[derive(Debug, Copy, Clone)]
pub struct Arguments {
    pub one: u64,
    pub two: u64,
    pub three: u64,
    pub four: u64,
    pub five: u64,
    pub six: u64,
}

#[must_use]
pub fn syscall(syscall: Syscall, args: &Arguments) -> SyscallReturnValue {
    match syscall {
        Syscall::Exit => sc_exit(args),
        Syscall::MemoryMap => SyscallReturnValue::ValueU(sc_mmap(args)),
        Syscall::Read => SyscallReturnValue::ValueI(sc_read(args)),
        Syscall::Write => SyscallReturnValue::ValueI(sc_write(args)),
        Syscall::Open => SyscallReturnValue::ValueI(sc_open(args)),
        Syscall::Close => SyscallReturnValue::Code(sc_close(args)),
        Syscall::Sleep => SyscallReturnValue::Code(sc_sleep(args)),

        Syscall::Invalid => SyscallReturnValue::Code(SyscallExitCode::Failure),
    }
}

fn sc_exit(args: &Arguments) -> ! {
    #[cfg_attr(not(debug_assertions), allow(unused_variables))]
    let exit_code = args.one;

    #[cfg(debug_assertions)]
    {
        let exit_code = beskar_core::syscall::ExitCode::try_from(exit_code);
        let tid = crate::process::scheduler::current_thread_id();

        if let Ok(exit_code) = exit_code {
            video::debug!("Thread {} exited with code {:?}", tid.as_u64(), exit_code);
        } else {
            video::debug!("Thread {} exited with invalid code", tid.as_u64());
        }
    }

    unsafe { crate::process::scheduler::exit_current_thread() }
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

    let Some(page_range) = process::current()
        .address_space()
        .with_pgalloc(|palloc| palloc.allocate_pages::<M4KiB>(len.div_ceil(M4KiB::SIZE)))
    else {
        return 0;
    };

    let success = crate::mem::frame_alloc::with_frame_allocator(|fralloc| {
        process::current().address_space().with_page_table(|kpt| {
            for page in page_range {
                let Some(frame) = fralloc.alloc() else {
                    return false;
                };
                kpt.map(
                    page,
                    frame,
                    Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
                    fralloc,
                )
                .flush();
            }
            true
        })
    });
    if !success {
        return 0;
    }

    debug_assert!(process::current().address_space().is_addr_owned(
        page_range.start().start_address(),
        page_range.end().start_address() + (len - 1),
    ));

    // FIXME: Should the area be zeroed?

    page_range.start().start_address().as_u64()
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

    if !process::current()
        .address_space()
        .is_addr_owned(buffer_start, buffer_start + buffer_len)
    {
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

    if !process::current()
        .address_space()
        .is_addr_owned(buffer_start, buffer_start + buffer_len)
    {
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

    if !process::current()
        .address_space()
        .is_addr_owned(path_start, path_start + path_len)
    {
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

    // FIXME: Put the thread to sleep
    crate::time::wait(sleep_time);

    SyscallExitCode::Success
}
