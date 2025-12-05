//! Frame allocator
//!
//! A frame allocator should allow the allocation of physical frames and keep track of the
//! allocated frames. It should also provide a way to free frames.
//!
//! Allocated frames do not need to be contiguous.

use beskar_core::arch::{
    PhysAddr,
    paging::{Frame, M4KiB, MemSize},
};
use beskar_core::mem::ranges::{MemoryRange, MemoryRanges};
use hyperdrive::locks::mcs::McsLock;

const MAX_MEMORY_REGIONS: usize = 4096;

static KFRAME_ALLOC: McsLock<FrameAllocator> = McsLock::new(FrameAllocator {
    memory_ranges: MemoryRanges::new(),
});

pub fn init(ranges: &[MemoryRange]) {
    assert!(!ranges.is_empty(), "No usable memory regions found");
    if ranges.len() >= MAX_MEMORY_REGIONS {
        video::warn!(
            "Too many usable memory regions, using only the first {}",
            MAX_MEMORY_REGIONS
        );
    }

    KFRAME_ALLOC.with_locked(|frallocator| {
        ranges.iter().take(MAX_MEMORY_REGIONS).for_each(|r| {
            frallocator.memory_ranges.insert(*r);
        });
        video::info!(
            "Free memory: {} MiB",
            frallocator.memory_ranges.sum() / 1_048_576
        );

        // Make sure physical frame for the AP trampoline code is reserved
        reserve_tramp_frame(frallocator);
    });
}

pub struct FrameAllocator {
    memory_ranges: MemoryRanges<MAX_MEMORY_REGIONS>,
}

impl FrameAllocator {
    #[must_use]
    #[inline]
    /// Allocate a frame anywhere in memory
    pub fn alloc<S: MemSize>(&mut self) -> Option<Frame<S>> {
        let size = S::SIZE;
        let alignment = S::SIZE;

        let addr = self.memory_ranges.allocate(size, alignment)?;
        let paddr = PhysAddr::new_truncate(u64::try_from(addr).unwrap());
        let frame = Frame::containing_address(paddr);
        Some(frame)
    }

    #[must_use]
    /// Allocate a frame according to a specific request.
    pub fn alloc_request<S: MemSize, const M: usize>(
        &mut self,
        req_ranges: &MemoryRanges<M>,
    ) -> Option<Frame<S>> {
        let size = S::SIZE;
        let alignment = S::SIZE;

        let addr = self
            .memory_ranges
            .allocate_req(size, alignment, req_ranges)?;
        let paddr = PhysAddr::new_truncate(u64::try_from(addr).unwrap());
        let frame = Frame::containing_address(paddr);
        Some(frame)
    }

    /// Free a frame
    pub fn free<S: MemSize>(&mut self, frame: Frame<S>) {
        self.memory_ranges.insert(MemoryRange::new(
            frame.start_address().as_u64(),
            frame.start_address().as_u64() + (frame.size() - 1),
        ));
    }
}

impl<S: MemSize> beskar_core::arch::paging::FrameAllocator<S> for FrameAllocator {
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
        .alloc_request::<M4KiB, 1>(&req_range)
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
