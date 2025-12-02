use crate::mem::{address_space, frame_alloc};
use beskar_core::arch::paging::{CacheFlush as _, M2MiB, Mapper as _, MemSize, PageRangeInclusive};
use beskar_hal::paging::page_table::Flags;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};
use hyperdrive::locks::mcs::MUMcsLock;

/// Number of 2 MiB pages to allocate for the kernel heap.
const KERNEL_HEAP_PAGES: u64 = 4; // 8 MiB

static KERNEL_HEAP: MUMcsLock<Heap> = MUMcsLock::uninit();

#[global_allocator]
static GLOBAL_ALLOCATOR: HeapGA = HeapGA;

pub fn init() {
    let page_range = address_space::with_kernel_pgalloc(|page_allocator| {
        page_allocator.allocate_pages(KERNEL_HEAP_PAGES).unwrap()
    });

    video::debug!(
        "Kernel heap allocated at {:#x}",
        page_range.start().start_address().as_u64()
    );

    frame_alloc::with_frame_allocator(|frame_allocator| {
        super::address_space::with_kernel_pt(|page_table| {
            for page in page_range {
                let frame = frame_allocator.alloc::<M2MiB>().unwrap();
                page_table
                    .map(
                        page,
                        frame,
                        Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                        frame_allocator,
                    )
                    .flush();
            }
        });
    });

    KERNEL_HEAP.init(Heap::new(page_range).unwrap());
}

struct Heap {
    heap: heaperion::Heap,
}

impl Heap {
    pub fn new(page_range: PageRangeInclusive<M2MiB>) -> heaperion::Result<Self> {
        let start_address = page_range.start().start_address().as_u64();
        let end_address = page_range.end().start_address().as_u64() + (M2MiB::SIZE - 1);

        let size = usize::try_from(end_address - start_address + 1).unwrap();
        let heap =
            unsafe { heaperion::Heap::new(page_range.start().start_address().as_mut_ptr(), size) }?;

        Ok(Self { heap })
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        self.heap
            .allocate(layout)
            .ok()
            .map_or(core::ptr::null_mut(), core::ptr::NonNull::as_ptr)
    }

    pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        if let Some(ptr) = NonNull::new(ptr) {
            let res = unsafe { self.heap.deallocate(ptr, layout) };
            debug_assert!(res.is_ok(), "Heap deallocation failed: {:?}", res);
        }
    }
}

/// A struct that is used as a global allocator.
///
/// It uses the static kernel heap to allocate memory.
struct HeapGA;

unsafe impl GlobalAlloc for HeapGA {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        KERNEL_HEAP
            .with_locked_if_init(|heap| heap.alloc(layout))
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        // Safety:
        // `GlobalAlloc` guarantees that the pointer is valid and the layout is correct.
        KERNEL_HEAP.with_locked_if_init(|heap| unsafe { heap.dealloc(ptr, layout) });
    }
}
