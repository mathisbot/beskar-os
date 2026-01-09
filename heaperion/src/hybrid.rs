//! Hybrid heap allocator
//!
//! Combines slab and buddy allocators for optimal performance across different
//! allocation sizes. Small allocations use the slab allocator for O(1) operations,
//! while larger allocations use the buddy allocator for better memory efficiency.

use core::alloc::Layout;
use core::ptr::NonNull;

use crate::buddy::BuddyAllocator;
use crate::error::{HeapError, Result};
use crate::slab::SlabAllocator;

/// Threshold for choosing between slab and buddy allocator
/// Allocations smaller than this use slab, larger use buddy
const SLAB_THRESHOLD: usize = 512;

/// A hybrid allocator that combines slab and buddy allocators
///
/// This allocator provides the best of both worlds:
/// - Fast O(1) allocation for small objects via slab allocator
/// - Efficient memory usage for large objects via buddy allocator
/// - Minimal fragmentation through intelligent strategy selection
pub struct HybridAllocator {
    /// Slab allocator for small allocations
    slab: SlabAllocator,
    /// Buddy allocator for large allocations
    buddy: BuddyAllocator,
}

impl HybridAllocator {
    /// Create a new hybrid allocator
    ///
    /// The memory will be divided between slab and buddy allocators.
    /// Approximately 25% goes to slab for small allocations,
    /// and 75% goes to buddy for larger allocations.
    ///
    /// # Safety
    ///
    /// - `ptr` must be valid for reads and writes for `size` bytes
    /// - The memory region must not be used by other code while the allocator is active
    ///
    /// # Errors
    ///
    /// - `HeapError::InvalidSize` if the provided size is too small to initialize both allocators
    pub unsafe fn new(heap_start: *mut u8, heap_size: usize) -> Result<Self> {
        // Divide memory: 25% for slab, 75% for buddy
        let slab_size = heap_size / 4;
        let buddy_size = heap_size - slab_size;
        let buddy_start = unsafe { heap_start.add(slab_size) };

        Ok(Self {
            slab: unsafe { SlabAllocator::new(heap_start, slab_size) }?,
            buddy: unsafe { BuddyAllocator::new(buddy_start, buddy_size) }?,
        })
    }

    /// Allocate memory with the given layout
    ///
    /// Small allocations (< 512 bytes) are handled by the slab allocator
    /// for fast O(1) performance. Larger allocations use the buddy allocator
    /// for better memory efficiency.
    ///
    /// # Errors
    ///
    /// - `HeapError::InvalidSize` if the requested size is zero
    pub fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>> {
        if layout.size() == 0 {
            return Err(HeapError::InvalidSize);
        }

        // Choose allocator based on size
        if layout.size() <= SLAB_THRESHOLD && layout.size() <= self.slab.max_size() {
            // Try slab first for small allocations
            if let Ok(ptr) = self.slab.allocate(layout) {
                return Ok(ptr);
            }
            // Fall back to buddy if slab is full
        }

        // Use buddy for large allocations or when slab fails
        self.buddy.allocate(layout)
    }

    /// Deallocate memory at the given pointer with the given layout
    ///
    /// Automatically routes to the correct sub-allocator based on the size.
    ///
    /// # Safety
    ///
    /// - `ptr` must have been allocated by this allocator
    /// - `layout` must be the same as the layout used to allocate
    /// - `ptr` must not be deallocated more than once
    ///
    /// # Errors
    ///
    /// - `HeapError::InvalidPointer` if the pointer was not allocated by this allocator
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) -> Result<()> {
        // Try to deallocate from slab first for small sizes
        if layout.size() <= SLAB_THRESHOLD && layout.size() <= self.slab.max_size() {
            // SAFETY: Caller guarantees ptr was allocated by us
            if unsafe { self.slab.deallocate(ptr).is_ok() } {
                return Ok(());
            }
        }

        // Otherwise deallocate from buddy
        // SAFETY: Caller guarantees ptr was allocated by us
        unsafe { self.buddy.deallocate(ptr, layout) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate alloc;
    use alloc::vec::Vec;

    #[test]
    fn test_hybrid_init() {
        let mut buffer = alloc::vec![0u8; 16384];
        let allocator = unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) };

        assert!(allocator.is_ok());
    }

    #[test]
    fn test_hybrid_small_allocation() {
        let mut buffer = alloc::vec![0u8; 16_384];
        let mut allocator =
            unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        // Small allocation should use slab
        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = allocator.allocate(layout);
        assert!(ptr.is_ok());

        unsafe {
            allocator.deallocate(ptr.unwrap(), layout).unwrap();
        }
    }

    #[test]
    fn test_hybrid_large_allocation() {
        let mut buffer = alloc::vec![0u8; 32_768];
        let mut allocator =
            unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        // Large allocation should use buddy
        let layout = Layout::from_size_align(2048, 16).unwrap();
        let ptr = allocator.allocate(layout);
        assert!(ptr.is_ok());

        unsafe {
            allocator.deallocate(ptr.unwrap(), layout).unwrap();
        }
    }

    #[test]
    fn test_hybrid_mixed_allocations() {
        let mut buffer = alloc::vec![0u8; 65_536];
        let mut allocator =
            unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let small_layout = Layout::from_size_align(32, 8).unwrap();
        let medium_layout = Layout::from_size_align(256, 8).unwrap();
        let large_layout = Layout::from_size_align(4096, 16).unwrap();

        let ptr1 = allocator.allocate(small_layout).unwrap();
        let ptr2 = allocator.allocate(large_layout).unwrap();
        let ptr3 = allocator.allocate(medium_layout).unwrap();
        let ptr4 = allocator.allocate(small_layout).unwrap();

        // All pointers should be different
        assert_ne!(ptr1.as_ptr(), ptr2.as_ptr());
        assert_ne!(ptr2.as_ptr(), ptr3.as_ptr());
        assert_ne!(ptr3.as_ptr(), ptr4.as_ptr());
        assert_ne!(ptr1.as_ptr(), ptr3.as_ptr());

        unsafe {
            allocator.deallocate(ptr1, small_layout).unwrap();
            allocator.deallocate(ptr2, large_layout).unwrap();
            allocator.deallocate(ptr3, medium_layout).unwrap();
            allocator.deallocate(ptr4, small_layout).unwrap();
        }
    }

    #[test]
    fn test_hybrid_reallocation() {
        let mut buffer = alloc::vec![0u8; 32_768];
        let mut allocator =
            unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let layout = Layout::from_size_align(128, 8).unwrap();

        // Allocate and deallocate multiple times
        for _ in 0..5 {
            let ptr = allocator.allocate(layout).unwrap();
            unsafe {
                allocator.deallocate(ptr, layout).unwrap();
            }
        }
    }

    #[test]
    fn test_hybrid_stress() {
        let mut buffer = alloc::vec![0u8; 131_072];
        let mut allocator =
            unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let mut allocations = Vec::new();

        // Mix of small and large allocations
        for i in 0..20 {
            let size = if i % 3 == 0 {
                64
            } else if i % 3 == 1 {
                256
            } else {
                1024
            };
            let layout = Layout::from_size_align(size, 8).unwrap();

            if let Ok(ptr) = allocator.allocate(layout) {
                allocations.push((ptr, layout));
            }
        }

        // Deallocate all
        for (ptr, layout) in allocations {
            unsafe {
                allocator.deallocate(ptr, layout).unwrap();
            }
        }
    }

    #[test]
    fn test_hybrid_zero_size() {
        let mut buffer = alloc::vec![0u8; 16_384];
        let mut allocator =
            unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let layout = Layout::from_size_align(0, 8).unwrap();
        let result = allocator.allocate(layout);
        assert!(matches!(result, Err(HeapError::InvalidSize)));
    }
}
