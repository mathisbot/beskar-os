use crate::arch::syscalls;
use beskar_core::syscall::Syscall;
use core::ptr::NonNull;
use hyperdrive::locks::mcs::MUMcsLock;

static ALLOCATOR: MUMcsLock<linked_list_allocator::Heap> = MUMcsLock::uninit();

struct Heap;

#[global_allocator]
static HEAP: Heap = Heap;

pub(crate) const HEAP_SIZE: u64 = 1024 * 1024; // 1 MiB

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

#[must_use]
#[inline]
/// Map memory into the address space
///
/// ## Panics
///
/// Panics if the syscall fails.
pub fn mmap(size: u64) -> NonNull<u8> {
    let res = syscalls::syscall_1(Syscall::MemoryMap, size);
    NonNull::new(res as *mut u8).expect("Memory mapping failed")
}
