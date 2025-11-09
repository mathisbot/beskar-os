use crate::{
    arch::syscalls,
    error::{MemoryError, MemoryErrorKind, MemoryResult},
};
use beskar_core::{
    arch::paging::{M4KiB, MemSize as _},
    syscall::Syscall,
};
use core::{num::NonZeroU64, ptr::NonNull};
use hyperdrive::locks::mcs::MUMcsLock;

static ALLOCATOR: MUMcsLock<linked_list_allocator::Heap> = MUMcsLock::uninit();

struct Heap;

#[global_allocator]
static HEAP: Heap = Heap;

pub(crate) const HEAP_SIZE: u64 = 20 * 1024 * 1024; // 20 MiB
beskar_core::static_assert!(HEAP_SIZE.is_multiple_of(M4KiB::SIZE));

unsafe impl core::alloc::GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let Some(Ok(res)) = ALLOCATOR.with_locked_if_init(|heap| heap.allocate_first_fit(layout))
        else {
            return core::ptr::null_mut();
        };
        res.as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        ALLOCATOR.with_locked_if_init(|heap| unsafe {
            heap.deallocate(NonNull::new_unchecked(ptr), layout);
        });
    }
}

#[inline]
/// Initialize the heap allocator
pub(crate) unsafe fn init_heap(start: *mut u8, size: usize) {
    ALLOCATOR.init(unsafe { linked_list_allocator::Heap::new(start, size) });
}

/// Map memory into the address space
///
/// # Errors
///
/// Returns an error if the memory cannot be mapped.
pub fn mmap(size: u64, alignment: Option<NonZeroU64>) -> MemoryResult<NonNull<u8>> {
    if let Some(a) = alignment
        && !a.get().is_power_of_two()
    {
        return Err(MemoryError::new(MemoryErrorKind::InvalidAlignment));
    }

    let res = syscalls::syscall_2(
        Syscall::MemoryMap,
        size,
        alignment.map_or(1, NonZeroU64::get),
    );

    NonNull::new(res as *mut u8).ok_or_else(|| MemoryError::new(MemoryErrorKind::OutOfMemory))
}
