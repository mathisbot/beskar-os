//! TODO: Is having per-core heaps a good idea?

use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

use crate::mem::page_alloc;
use beskar_core::arch::commons::paging::{M2MiB, MemSize, PageRangeInclusive};
use beskar_core::arch::x86_64::paging::page_table::Flags;
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
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        );
    });

    KERNEL_HEAP.init(Heap::new(page_range));
}

struct Heap {
    // start: NonZeroU64,
    // end: NonZeroU64,
    // TODO: Add faster allocator
    linked_list: linked_list_allocator::Heap,
}

impl Heap {
    pub fn new(page_range: PageRangeInclusive<M2MiB>) -> Self {
        let start_address = page_range.start.start_address().as_u64();
        let end_address = page_range.end.start_address().as_u64() + (M2MiB::SIZE - 1);

        let size = usize::try_from(end_address - start_address + 1).unwrap();
        let linked_list = unsafe {
            linked_list_allocator::Heap::new(page_range.start.start_address().as_mut_ptr(), size)
        };

        Self {
            // start: start_address.try_into().unwrap(),
            // end: end_address.try_into().unwrap(),
            linked_list,
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        self.linked_list
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), core::ptr::NonNull::as_ptr)
    }

    pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let Some(ptr) = NonNull::new(ptr) else {
            return;
        };
        unsafe { self.linked_list.deallocate(ptr, layout) };
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
        // Safety:
        // `GlobalAlloc` guarantees that the pointer is valid and the layout is correct.
        KERNEL_HEAP.try_with_locked(|heap| unsafe { heap.dealloc(ptr, layout) });
    }
}
