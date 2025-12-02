//! Slab allocator implementation
//!
//! The slab allocator is optimized for frequent allocations and deallocations of
//! fixed-size objects. It maintains pools of pre-sized memory chunks, providing
//! O(1) allocation and deallocation with excellent cache locality.

use core::alloc::Layout;
use core::ptr::NonNull;

use crate::{
    error::{HeapError, Result},
    utils::align_up,
};

/// Size classes for the slab allocator
/// These are common allocation sizes that benefit from dedicated pools
const SLAB_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512];

/// Minimum heap size required for slab allocator
const MIN_HEAP_SIZE: usize = 4096;

/// Maximum number of slab size classes
const MAX_SLABS: usize = 8;

/// Represents a free slot in a slab
struct FreeSlot {
    next: Option<NonNull<Self>>,
}

/// A single slab that manages fixed-size allocations
struct Slab {
    /// Size of each slot in this slab
    slot_size: usize,
    /// Head of the free list
    free_list: Option<NonNull<FreeSlot>>,
    /// Start pointer of this slab's memory region
    start: *mut u8,
    /// End pointer of this slab's memory region
    end: *mut u8,
    /// Number of free slots
    free_count: usize,
    /// Total capacity
    capacity: usize,
}

impl Slab {
    /// Create a new slab
    ///
    /// # Safety
    ///
    /// - `ptr` must be valid for reads and writes for `size` bytes
    /// - The memory region must not be used by other code while the slab is active
    ///
    /// # Errors
    ///
    /// - `HeapError::InvalidSize` if the size is too small to hold any slots
    unsafe fn new(slot_size: usize, start_ptr: *mut u8, size: usize) -> Result<Self> {
        // Align start address to the slot size
        let addr = start_ptr.addr();
        let aligned_addr = align_up(addr, slot_size);
        let offset = aligned_addr - addr;

        if offset >= size {
            return Err(HeapError::InvalidSize);
        }

        let adjusted_size = size - offset;
        let aligned_ptr = unsafe { start_ptr.add(offset) };

        // Calculate how many slots fit in this region
        let num_slots = adjusted_size / slot_size;

        // Build the free list by linking all slots
        let mut current_ptr = aligned_ptr;
        let mut prev_slot: Option<NonNull<FreeSlot>> = None;

        let mut free_list = None;
        for _ in 0..num_slots {
            #[allow(clippy::cast_ptr_alignment)]
            let slot = current_ptr.cast::<FreeSlot>();

            // SAFETY: We own this memory region
            unsafe {
                (*slot).next = None;
            }

            let slot_ptr = unsafe { NonNull::new_unchecked(slot) };

            if let Some(prev) = prev_slot {
                // SAFETY: prev is from our owned memory region
                unsafe {
                    (*prev.as_ptr()).next = Some(slot_ptr);
                }
            } else {
                // This is the first slot, set it as the head
                free_list = Some(slot_ptr);
            }

            prev_slot = Some(slot_ptr);
            current_ptr = unsafe { current_ptr.add(slot_size) };
        }

        Ok(Self {
            slot_size,
            free_list,
            start: aligned_ptr,
            end: unsafe { aligned_ptr.add(adjusted_size) },
            free_count: num_slots,
            capacity: num_slots,
        })
    }

    /// Allocate a slot from this slab
    fn allocate(&mut self) -> Result<NonNull<u8>> {
        if let Some(slot) = self.free_list {
            // SAFETY: slot is from our free list
            let next = unsafe { (*slot.as_ptr()).next };
            self.free_list = next;
            self.free_count -= 1;

            // SAFETY: We're returning a pointer from our managed memory
            Ok(slot.cast::<u8>())
        } else {
            Err(HeapError::OutOfMemory)
        }
    }

    /// Deallocate a slot back to this slab
    ///
    /// # Safety
    ///
    /// - `ptr` must have been allocated from this slab
    /// - `ptr` must not be deallocated more than once
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>) -> Result<()> {
        let ptr_raw = ptr.as_ptr();

        // Verify the pointer is within our range
        if ptr_raw < self.start || ptr_raw >= self.end {
            return Err(HeapError::InvalidPointer);
        }

        // Verify alignment
        let offset = unsafe { ptr_raw.offset_from(self.start).cast_unsigned() };
        if !offset.is_multiple_of(self.slot_size) {
            return Err(HeapError::InvalidPointer);
        }

        let slot = ptr.cast::<FreeSlot>();

        // SAFETY: Caller guarantees this was a valid allocation
        unsafe {
            (*slot.as_ptr()).next = self.free_list;
        }

        self.free_list = Some(slot);
        self.free_count += 1;

        Ok(())
    }

    /// Check if a pointer belongs to this slab
    #[inline]
    fn contains(&self, ptr: *mut u8) -> bool {
        ptr >= self.start && ptr < self.end
    }

    /// Check if the slab is empty (all slots free)
    #[inline]
    const fn _is_empty(&self) -> bool {
        self.free_count == self.capacity
    }

    /// Check if the slab is full (no free slots)
    #[inline]
    const fn is_full(&self) -> bool {
        self.free_count == 0
    }
}

/// Slab allocator for managing small, fixed-size allocations
///
/// Maintains multiple slabs for different size classes, providing O(1)
/// allocation and deallocation for common small object sizes.
pub struct SlabAllocator {
    /// Array of slabs for different size classes
    slabs: [Option<Slab>; MAX_SLABS],
}

const NONE_SLAB: Option<Slab> = None;

impl SlabAllocator {
    /// Create a new slab allocator
    ///
    /// The memory region will be divided among the different slab size classes.
    ///
    /// # Safety
    ///
    /// - `heap_ptr` must point to valid memory
    /// - The region `[heap_ptr, heap_ptr.add(heap_size))` must be exclusively owned
    /// - The memory must remain valid for the lifetime of the allocator
    ///
    /// # Errors
    ///
    /// - `HeapError::AlreadyInitialized` if the allocator is already initialized
    /// - `HeapError::InvalidSize` if the provided size is too small
    pub unsafe fn new(ptr: *mut u8, size: usize) -> Result<Self> {
        // We need at least 4KB to make meaningful slab allocations
        if size < MIN_HEAP_SIZE {
            return Err(HeapError::InvalidSize);
        }

        let mut current_ptr = ptr;
        let remaining_size = size;

        // Divide memory among slabs proportionally
        let base_size = remaining_size / SLAB_SIZES.len();

        let mut slabs = [NONE_SLAB; MAX_SLABS];
        for (i, &size) in SLAB_SIZES.iter().enumerate() {
            if i >= MAX_SLABS {
                break;
            }

            // Calculate remaining space
            let used = unsafe { current_ptr.offset_from(ptr).cast_unsigned() };
            let slab_size = base_size.min(remaining_size.saturating_sub(used));

            if slab_size < size * 4 {
                // Not enough space for this slab - need at least 4 slots
                break;
            }

            // SAFETY: Caller guarantees the memory is valid
            let slab = unsafe { Slab::new(size, current_ptr, slab_size) }?;

            // Only add the slab if it was successfully initialized
            if slab.capacity > 0 {
                slabs[i] = Some(slab);
            }
            current_ptr = unsafe { current_ptr.add(slab_size) };
        }

        Ok(Self { slabs })
    }

    /// Allocate memory with the given layout
    ///
    /// # Errors
    ///
    /// - `HeapError::InvalidSize` if the requested size is zero
    pub fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>> {
        if layout.size() == 0 {
            return Err(HeapError::InvalidSize);
        }

        // Find the appropriate slab for this size
        let size = layout.size().max(layout.align());

        for slab in self.slabs.iter_mut().flatten() {
            if slab.slot_size >= size {
                return slab.allocate();
            }
        }

        Err(HeapError::OutOfMemory)
    }

    /// Deallocate memory at the given pointer
    ///
    /// # Safety
    ///
    /// - `ptr` must have been allocated by this allocator
    /// - `ptr` must not be deallocated more than once
    ///
    /// # Errors
    ///
    /// - `HeapError::InvalidPointer` if the pointer was not allocated by this allocator
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>) -> Result<()> {
        let ptr_raw = ptr.as_ptr();

        // Find which slab owns this pointer
        for slab in self.slabs.iter_mut().flatten() {
            if slab.contains(ptr_raw) {
                // SAFETY: Caller guarantees this was allocated from this slab
                return unsafe { slab.deallocate(ptr) };
            }
        }

        Err(HeapError::InvalidPointer)
    }

    /// Check if a given layout can be satisfied by the slab allocator
    #[must_use]
    pub fn can_allocate(&self, layout: &Layout) -> bool {
        if layout.size() == 0 {
            return false;
        }

        let size = layout.size().max(layout.align());

        for slab in self.slabs.iter().flatten() {
            if slab.slot_size >= size && !slab.is_full() {
                return true;
            }
        }

        false
    }

    /// Get the maximum size that can be allocated by the slab allocator
    #[must_use]
    pub const fn max_size(&self) -> usize {
        if let Some(size) = SLAB_SIZES.last() {
            *size
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate alloc;
    use alloc::vec::Vec;

    #[test]
    fn test_slab_init() {
        let mut buffer = [0u8; 8192];
        let allocator = unsafe { SlabAllocator::new(buffer.as_mut_ptr(), buffer.len()) };

        assert!(allocator.is_ok());
    }

    #[test]
    fn test_slab_allocate_small() {
        let mut buffer = [0u8; 8192];
        let mut allocator =
            unsafe { SlabAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let layout = Layout::from_size_align(16, 8).unwrap();
        let ptr = allocator.allocate(layout);
        assert!(ptr.is_ok());
    }

    #[test]
    fn test_slab_allocate_deallocate() {
        let mut buffer = [0u8; 8192];
        let mut allocator =
            unsafe { SlabAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let layout = Layout::from_size_align(32, 8).unwrap();
        let ptr = allocator.allocate(layout).unwrap();

        unsafe {
            allocator.deallocate(ptr).unwrap();
        }

        // Should be able to allocate again
        let ptr2 = allocator.allocate(layout);
        assert!(ptr2.is_ok());
    }

    #[test]
    fn test_slab_multiple_allocations() {
        let mut buffer = [0u8; 16384];
        let mut allocator =
            unsafe { SlabAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let mut ptrs = Vec::new();

        for _ in 0..10 {
            let layout = Layout::from_size_align(64, 8).unwrap();
            let ptr = allocator.allocate(layout).unwrap();
            ptrs.push(ptr);
        }

        // All pointers should be different
        for i in 0..ptrs.len() {
            for j in (i + 1)..ptrs.len() {
                assert_ne!(ptrs[i].as_ptr(), ptrs[j].as_ptr());
            }
        }

        // Deallocate all
        for ptr in ptrs {
            unsafe {
                allocator.deallocate(ptr).unwrap();
            }
        }
    }

    #[test]
    fn test_slab_can_allocate() {
        let mut buffer = [0u8; 8192];
        let allocator = unsafe { SlabAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let small_layout = Layout::from_size_align(32, 8).unwrap();
        assert!(allocator.can_allocate(&small_layout));

        let large_layout = Layout::from_size_align(1024, 8).unwrap();
        assert!(!allocator.can_allocate(&large_layout));
    }
}
