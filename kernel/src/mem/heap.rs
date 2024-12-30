use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    ptr::NonNull,
};

use x86_64::structures::paging::{page::PageRangeInclusive, PageSize, PageTableFlags, Size2MiB};

use crate::mem::page_alloc;
use hyperdrive::locks::mcs::MUMcsLock;

use super::frame_alloc;

/// Number of 2 MiB pages to allocate for the kernel heap.
const KERNEL_HEAP_PAGES: u64 = 4; // 8 MiB

static KERNEL_HEAP: MUMcsLock<Heap> = MUMcsLock::uninit();

#[global_allocator]
static GLOBAL_ALLOCATOR: HeapGA = HeapGA;

pub fn init() {
    let page_range = page_alloc::with_page_allocator(|page_allocator| {
        page_allocator.allocate_pages(KERNEL_HEAP_PAGES).unwrap()
    });

    frame_alloc::with_frame_allocator(|frame_allocator| {
        frame_allocator.map_pages(
            page_range,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
        );
    });

    KERNEL_HEAP.init(Heap::new(page_range));
}

struct Heap {
    // start: NonZeroU64,
    // end: NonZeroU64,
    // TODO: Add faster allocator
    linked_list: UnsafeCell<linked_list_allocator::Heap>,
}

impl Heap {
    pub fn new(page_range: PageRangeInclusive<Size2MiB>) -> Self {
        let start_address = page_range.start.start_address().as_u64();
        let end_address = page_range.end.start_address().as_u64() + (Size2MiB::SIZE - 1);

        let size = usize::try_from(end_address - start_address + 1).unwrap();
        let linked_list = unsafe {
            linked_list_allocator::Heap::new(page_range.start.start_address().as_mut_ptr(), size)
        };

        Self {
            // start: start_address.try_into().unwrap(),
            // end: end_address.try_into().unwrap(),
            linked_list: UnsafeCell::new(linked_list),
        }
    }

    pub fn alloc(&self, layout: Layout) -> *mut u8 {
        // According to the safety requirements of the `GlobalAlloc trait`, we need to ensure that
        // this function doesn't panic. Therefore, we need to return null if the allocation fails.

        // Safety:
        // The heap is locked, so we can safely access its fields.
        let linked_allocator = unsafe { &mut *self.linked_list.get() };

        linked_allocator
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), core::ptr::NonNull::as_ptr)
    }

    pub fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }
        // Panics:
        // The pointer is not null.
        let non_null_ptr = NonNull::new(ptr).unwrap();

        // Safety:
        // The heap is locked, so we can safely access its fields.
        let linked_allocator = unsafe { &mut *self.linked_list.get() };

        // Safety:
        // `GlobalAlloc` guarantees that the pointer is valid and the layout is correct.
        unsafe { linked_allocator.deallocate(non_null_ptr, layout) };
    }
}

/// A struct that is used as a global allocator.
///
/// It uses the static kernel heap to allocate memory.
struct HeapGA;

unsafe impl GlobalAlloc for HeapGA {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        KERNEL_HEAP
            .try_with_locked(|heap| heap.alloc(layout))
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        KERNEL_HEAP.try_with_locked(|heap| heap.dealloc(ptr, layout));
    }
}
