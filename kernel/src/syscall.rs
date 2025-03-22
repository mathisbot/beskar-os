use crate::process;
use beskar_core::{
    arch::commons::{
        VirtAddr,
        paging::{CacheFlush as _, Flags, M4KiB, Mapper as _, MemSize},
    },
    syscall::{Syscall, SyscallExitCode, SyscallReturnValue},
};

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
        Syscall::Print => SyscallReturnValue::Code(sc_print(args)),
        Syscall::Exit => sc_exit(args),
        Syscall::RandomGen => SyscallReturnValue::Code(sc_randomgen(args)),
        Syscall::MemoryMap => SyscallReturnValue::Value(sc_mmap(args)),
        Syscall::Invalid => SyscallReturnValue::Code(SyscallExitCode::Failure),
    }
}

#[must_use]
fn sc_print(args: &Arguments) -> SyscallExitCode {
    let Some(msg_vaddr) = VirtAddr::try_new(args.one) else {
        return SyscallExitCode::Failure;
    };

    let msg_addr = msg_vaddr.as_ptr();
    let msg_len = args.two;

    if !process::current()
        .address_space()
        .is_addr_owned(msg_vaddr, msg_vaddr + msg_len)
    {
        return SyscallExitCode::Failure;
    }

    let buf = unsafe { core::slice::from_raw_parts(msg_addr, msg_len.try_into().unwrap()) };
    let Ok(msg) = core::str::from_utf8(buf) else {
        return SyscallExitCode::Failure;
    };

    let tid = crate::process::scheduler::current_thread_id();
    crate::info!("[Thread {}] {}", tid.as_u64(), msg);
    SyscallExitCode::Success
}

fn sc_exit(args: &Arguments) -> ! {
    #[cfg_attr(not(debug_assertions), allow(unused_variables))]
    let exit_code = args.one;

    #[cfg(debug_assertions)]
    {
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

fn sc_randomgen(args: &Arguments) -> SyscallExitCode {
    let Some(start_vaddr) = VirtAddr::try_new(args.one) else {
        return SyscallExitCode::Failure;
    };

    let start_addr = start_vaddr.as_mut_ptr();
    let len = args.two;

    if !process::current()
        .address_space()
        .is_addr_owned(start_vaddr, start_vaddr + len)
    {
        return SyscallExitCode::Failure;
    }

    let buffer = unsafe { core::slice::from_raw_parts_mut(start_addr, len.try_into().unwrap()) };

    let rand_res = crate::arch::rand::rand_bytes(buffer);

    match rand_res {
        Ok(()) => SyscallExitCode::Success,
        Err(_) => SyscallExitCode::Failure,
    }
}

fn sc_mmap(args: &Arguments) -> u64 {
    let len = args.one;

    let Some(page_range) = process::current()
        .address_space()
        .with_pgalloc(|palloc| palloc.allocate_pages::<M4KiB>(len.div_ceil(M4KiB::SIZE)))
    else {
        return 0;
    };

    crate::mem::frame_alloc::with_frame_allocator(|fralloc| {
        process::current().address_space().with_page_table(|kpt| {
            for page in page_range {
                let frame = fralloc.alloc().unwrap(); // TODO: Handle allocation failure
                kpt.map(
                    page,
                    frame,
                    Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
                    fralloc,
                )
                .flush();
            }
        });
    });

    // FIXME: Should the area be zeroed?

    page_range.start.start_address().as_u64()
}
