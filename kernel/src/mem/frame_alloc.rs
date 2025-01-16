//! Frame allocator
//!
//! A frame allocator should allow the allocation of physical frames and keep track of the
//! allocated frames. It should also provide a way to free frames.
//!
//! Allocated frames do not need to be contiguous.

use super::{
    page_table,
    ranges::{MemoryRange, MemoryRangeRequest, MemoryRanges},
};
use bootloader::structs::{MemoryRegion, MemoryRegionUsage};
use x86_64::{
    structures::paging::{
        page::PageRangeInclusive, Mapper, PageSize, PageTableFlags, PhysFrame, RecursivePageTable, Size4KiB
    }, PhysAddr
};

use hyperdrive::locks::mcs::MUMcsLock;

const MAX_MEMORY_REGIONS: usize = 1024;

static KFRAME_ALLOC: MUMcsLock<FrameAllocator> = MUMcsLock::uninit();

pub fn init(regions: &[MemoryRegion]) {
    // Count usable memory regions
    let mut usable_regions = 0;
    for region in regions {
        if region.kind == MemoryRegionUsage::Usable {
            usable_regions += 1;
        }
    }
    // FIXME: Two stages init for dynamic sizing?
    assert!(usable_regions > 0, "No usable memory regions found");
    if usable_regions >= MAX_MEMORY_REGIONS {
        crate::warn!(
            "[WARN ] Too many usable memory regions, using only the first {}",
            MAX_MEMORY_REGIONS
        );
    }

    let ranges = MemoryRanges::<MAX_MEMORY_REGIONS>::from_regions(regions);

    crate::info!("Free memory: {} MiB", ranges.sum() / 1_048_576);

    let mut frallocator = FrameAllocator {
        memory_ranges: ranges,
    };

    // Make sure physical frame for the AP trampoline code is reserved
    reserve_tramp_frame(&mut frallocator);

    KFRAME_ALLOC.init(frallocator);
}

pub struct FrameAllocator {
    memory_ranges: MemoryRanges<MAX_MEMORY_REGIONS>,
}

impl FrameAllocator {
    #[must_use]
    #[inline]
    pub fn alloc<S: PageSize>(&mut self) -> Option<PhysFrame<S>> {
        self.alloc_request(&MemoryRangeRequest::<MAX_MEMORY_REGIONS>::DontCare)
    }

    #[must_use]
    #[inline]
    pub fn alloc_request<S: PageSize, const M: usize>(
        &mut self,
        req_range: &MemoryRangeRequest<M>,
    ) -> Option<PhysFrame<S>> {
        let size = S::SIZE;
        let alignment = S::SIZE;

        let addr = self.memory_ranges.allocate(size, alignment, req_range)?;
        Some(PhysFrame::from_start_address(PhysAddr::new(u64::try_from(addr).unwrap())).unwrap())
    }

    // FIXME: Keep track of allocated frames?
    // Here nothing ensures the caller has provided a valid frame
    // (valid as in "usable memory region provided by the bootloader")
    pub fn free<S: PageSize>(&mut self, frame: PhysFrame<S>) {
        self.memory_ranges.insert(MemoryRange::new(
            frame.start_address().as_u64(),
            frame.start_address().as_u64() + (frame.size() - 1),
        ));
    }

    /// Given a range of pages, allocate whatever frames are needed and map them to the pages.
    pub fn map_pages<S: PageSize + core::fmt::Debug>(
        &mut self,
        pages: PageRangeInclusive<S>,
        flags: PageTableFlags,
    ) where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        page_table::with_page_table(|page_table| {
            for page in pages {
                let frame = self.alloc::<S>().unwrap();
                unsafe {
                    page_table.map_to_with_table_flags(
                        page,
                        frame,
                        flags,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        self,
                    )
                }
                .unwrap()
                .flush();
            }
        });
    }
}

unsafe impl<S: PageSize> x86_64::structures::paging::FrameAllocator<S> for FrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        self.alloc::<S>()
    }
}

/// Reserve a frame for the AP trampoline code
///
/// It is easier to allocate the frame at the beginning of memory initialization,
/// because we are sure that the needed region is available.
fn reserve_tramp_frame(allocator: &mut FrameAllocator) {
    let mut req_range = MemoryRanges::new();
    req_range.insert(MemoryRange::new(
        crate::arch::ap::AP_TRAMPOLINE_PADDR,
        crate::arch::ap::AP_TRAMPOLINE_PADDR + Size4KiB::SIZE,
    ));

    let _frame = allocator
        .alloc_request::<Size4KiB, 1>(&MemoryRangeRequest::MustBeWithin(&req_range))
        .expect("Failed to allocate AP frame");
}

#[inline]
/// Perform a single operation on the kernel frame allocator
pub fn with_frame_allocator<F, R>(f: F) -> R
where
    F: FnOnce(&mut FrameAllocator) -> R,
{
    KFRAME_ALLOC.with_locked(f)
}
