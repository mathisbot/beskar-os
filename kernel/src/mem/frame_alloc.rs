//! Frame allocator
//!
//! A frame allocator should allow the allocation of physical frames and keep track of the
//! allocated frames. It should also provide a way to free frames.
//!
//! Allocated frames do not need to be contiguous.

use beskar_core::arch::commons::{
    PhysAddr,
    paging::{Frame, M4KiB, MemSize},
};
use beskar_core::mem::{
    MemoryRegion, MemoryRegionUsage,
    ranges::{MemoryRange, MemoryRangeRequest, MemoryRanges},
};

use hyperdrive::locks::mcs::MUMcsLock;

const MAX_MEMORY_REGIONS: usize = 256;

static KFRAME_ALLOC: MUMcsLock<FrameAllocator> = MUMcsLock::uninit();

pub fn init(regions: &[MemoryRegion]) {
    // Count usable memory regions
    let mut usable_regions = 0;
    for region in regions {
        if region.kind() == MemoryRegionUsage::Usable {
            usable_regions += 1;
        }
    }
    assert!(usable_regions > 0, "No usable memory regions found");
    if usable_regions >= MAX_MEMORY_REGIONS {
        crate::warn!(
            "Too many usable memory regions, using only the first {}",
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
    /// Allocate a frame anywhere in memory
    pub fn alloc<S: MemSize>(&mut self) -> Option<Frame<S>> {
        self.alloc_request(&MemoryRangeRequest::<1>::DontCare)
    }

    #[must_use]
    #[inline]
    /// Allocate a frame according to a specific request.
    pub fn alloc_request<S: MemSize, const M: usize>(
        &mut self,
        req_range: &MemoryRangeRequest<M>,
    ) -> Option<Frame<S>> {
        let size = S::SIZE;
        let alignment = S::SIZE;

        let addr = self.memory_ranges.allocate(size, alignment, req_range)?;
        Some(Frame::from_start_address(PhysAddr::new(u64::try_from(addr).unwrap())).unwrap())
    }

    /// Free a frame
    pub fn free<S: MemSize>(&mut self, frame: Frame<S>) {
        self.memory_ranges.insert(MemoryRange::new(
            frame.start_address().as_u64(),
            frame.start_address().as_u64() + (frame.size() - 1),
        ));
    }
}

impl<S: MemSize> beskar_core::arch::commons::paging::FrameAllocator<S> for FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame<S>> {
        self.alloc::<S>()
    }

    fn deallocate_frame(&mut self, frame: Frame<S>) {
        self.free(frame);
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
        crate::arch::ap::AP_TRAMPOLINE_PADDR + M4KiB::SIZE,
    ));

    let _frame = allocator
        .alloc_request::<M4KiB, 1>(&MemoryRangeRequest::MustBeWithin(&req_range))
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
