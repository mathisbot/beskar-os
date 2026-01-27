use crate::error::{MemoryError, MemoryErrorKind, MemoryResult};
use beskar_core::arch::paging::{M4KiB, MemSize as _};
use core::{num::NonZeroU64, ptr::NonNull};
use hyperdrive::locks::mcs::MUMcsLock;

static ALLOCATOR: MUMcsLock<heaperion::Heap> = MUMcsLock::uninit();

struct Heap;

#[global_allocator]
static HEAP: Heap = Heap;

pub(crate) const HEAP_SIZE: u64 = 20 * 1024 * 1024; // 20 MiB
beskar_core::static_assert!(HEAP_SIZE.is_multiple_of(M4KiB::SIZE));

unsafe impl core::alloc::GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let Some(Ok(res)) = ALLOCATOR.with_locked_if_init(|heap| heap.allocate(layout)) else {
            return core::ptr::null_mut();
        };
        res.as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        ALLOCATOR.with_locked_if_init(|heap| unsafe {
            let _ = heap.deallocate(NonNull::new_unchecked(ptr), layout);
        });
    }
}

#[inline]
/// Initialize the heap allocator
pub(crate) unsafe fn init_heap(start: *mut u8, size: usize) {
    ALLOCATOR.init(unsafe { heaperion::Heap::new(start, size) }.unwrap());
}

/// Map memory into the address space
///
/// # Errors
///
/// Returns an error if the memory cannot be mapped.
pub fn mmap(
    size: u64,
    alignment: Option<NonZeroU64>,
    flags: MemoryProtection,
) -> MemoryResult<NonNull<u8>> {
    if alignment.is_some_and(|a| !a.get().is_power_of_two()) {
        return Err(MemoryError::new(MemoryErrorKind::InvalidAlignment));
    }

    let ptr = crate::sys::sc_mmap(size, alignment.map_or(1, NonZeroU64::get), flags as _);

    NonNull::new(ptr).ok_or_else(|| MemoryError::new(MemoryErrorKind::OutOfMemory))
}

/// Change the protection of a memory region
///
/// Returns true if the operation was successful, false otherwise.
///
/// Note that the pointer and size must be page-aligned.
pub fn mprotect(ptr: *mut u8, size: u64, flags: MemoryProtection) -> bool {
    let res = crate::sys::sc_mprotect(ptr, size, flags as _);
    res.is_success()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u64)]
pub enum MemoryProtection {
    NoAccess = 0,
    ReadOnly = beskar_core::syscall::consts::MFLAGS_READ,
    ReadWrite =
        beskar_core::syscall::consts::MFLAGS_READ | beskar_core::syscall::consts::MFLAGS_WRITE,
    ReadExecute =
        beskar_core::syscall::consts::MFLAGS_READ | beskar_core::syscall::consts::MFLAGS_EXECUTE,
    ReadWriteExecute = beskar_core::syscall::consts::MFLAGS_READ
        | beskar_core::syscall::consts::MFLAGS_WRITE
        | beskar_core::syscall::consts::MFLAGS_EXECUTE,
}
