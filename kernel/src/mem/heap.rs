use beskar_core::arch::paging::{M2MiB, MemSize as _};
use beskar_hal::paging::page_table::Flags;
use core::{alloc::GlobalAlloc, ptr::NonNull};
use heaperion::Heap;
use hyperdrive::locks::mcs::MUMcsLock;

/// Number of 2 MiB pages to allocate for the kernel heap.
const KERNEL_HEAP_PAGES: u64 = 4; // 8 MiB

static KERNEL_HEAP: MUMcsLock<Heap> = MUMcsLock::uninit();

#[global_allocator]
static GLOBAL_ALLOCATOR: HeapGA = HeapGA;

pub fn init() {
    let page_range = super::address_space::get_kernel_address_space()
        .alloc_map::<M2MiB>(
            usize::try_from(KERNEL_HEAP_PAGES * M2MiB::SIZE).unwrap(),
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        )
        .unwrap();

    video::debug!(
        "Kernel heap allocated at {:#x}",
        page_range.start().start_address().as_u64()
    );

    KERNEL_HEAP.init(
        unsafe {
            Heap::new(
                page_range.start().start_address().as_mut_ptr(),
                usize::try_from(page_range.size()).unwrap(),
            )
        }
        .unwrap(),
    );
}

/// A struct that is used as a global allocator.
///
/// It uses the static kernel heap to allocate memory.
struct HeapGA;

unsafe impl GlobalAlloc for HeapGA {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        KERNEL_HEAP
            .with_locked_if_init(|heap| heap.allocate(layout).ok())
            .flatten()
            .map_or(core::ptr::null_mut(), core::ptr::NonNull::as_ptr)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        // Safety: `ptr` is guaranteed to be valid as it was returned by `alloc`.
        let ptr = unsafe { NonNull::new_unchecked(ptr) };
        // Safety: `GlobalAlloc` guarantees that the pointer is valid and the layout is correct.
        KERNEL_HEAP.with_locked_if_init(|heap| {
            let _ = unsafe { heap.deallocate(ptr, layout) };
        });
    }
}
