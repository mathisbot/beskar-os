use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

use x86_64::structures::paging::{page::PageRangeInclusive, PageSize, PageTableFlags, Size4KiB};

use crate::{
    mem::page_alloc,
    utils::locks::{MUMcsLock, McsLock, McsNode},
};

use super::frame_alloc;

const KERNEL_HEAP_PAGES: u64 = 256; // 1 MiB

static KERNEL_HEAP: MUMcsLock<Heap> = MUMcsLock::uninit();

#[global_allocator]
static GLOBAL_ALLOCATOR: HeapGA = HeapGA;

pub fn init() {
    let page_range = page_alloc::with_page_allocator(|page_allocator| {
        page_allocator
            .allocate_pages::<Size4KiB>(KERNEL_HEAP_PAGES)
            .unwrap()
    });

    frame_alloc::with_frame_allocator(|frame_allocator| {
        frame_allocator.map_pages(
            page_range,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
        );
    });

    KERNEL_HEAP.init(Heap::new(page_range));
}

// FIXME: Having locks inside of the heap isn't needed,
// as it is already locked by the global.
pub struct Heap {
    // start: NonZeroU64,
    // end: NonZeroU64,
    // TODO: Add faster allocator
    linked_list: McsLock<linked_list_allocator::Heap>,
}

impl Heap {
    pub fn new(page_range: PageRangeInclusive<Size4KiB>) -> Self {
        let start_address = page_range.start.start_address().as_u64();
        let end_address = page_range.end.start_address().as_u64() + (Size4KiB::SIZE - 1);

        let size = usize::try_from(end_address - start_address + 1).unwrap();
        let linked_allocator = unsafe {
            linked_list_allocator::Heap::new(page_range.start.start_address().as_mut_ptr(), size)
        };

        Self {
            // start: start_address.try_into().unwrap(),
            // end: end_address.try_into().unwrap(),
            linked_list: McsLock::new(linked_allocator),
        }
    }

    pub fn alloc(&self, layout: Layout) -> *mut u8 {
        // According to the safety requirements of the `GlobalAlloc trait`, we need to ensure that
        // this function doesn't panic. Therefore, we need to return null if the allocation fails.
        self.linked_list
            .with_locked(|allocator| allocator.allocate_first_fit(layout))
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
        // `GlobalAlloc` guarantees that the pointer is valid and the layout is correct.
        self.linked_list
            .with_locked(|allocator| unsafe { allocator.deallocate(non_null_ptr, layout) });
    }
}

/// A struct that is used as a global allocator.
///
/// It uses the static kernel heap to allocate memory.
pub struct HeapGA;

unsafe impl GlobalAlloc for HeapGA {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let node = McsNode::new();
        // According to the `GlobalAlloc` trait, we need to ensure that this function doesn't panic.
        let allocator = KERNEL_HEAP.lock_if_init(&node);
        allocator.map_or(core::ptr::null_mut(), |heap| heap.alloc(layout))
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let node = McsNode::new();
        // According to the `GlobalAlloc` trait, we need to ensure that this function doesn't panic.
        let heap = KERNEL_HEAP.lock_if_init(&node);
        if let Some(heap) = heap {
            heap.dealloc(ptr, layout);
        }
    }
}
