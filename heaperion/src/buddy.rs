//! Buddy allocator implementation
//!
//! The buddy allocator splits memory into power-of-two sized blocks and manages them
//! using a tree structure. This provides good performance with O(log(n)) allocation and
//! deallocation, while minimizing external fragmentation.
use crate::utils::{align_up, order_to_size, size_to_order};
use crate::{HeapError, Result};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Maximum order (`2^MAX_ORDER` bytes is the largest block size)
const MAX_ORDER: usize = 28; // Up to 256 MiB blocks

/// Minimum block size (must be at least size of `FreeBlock`)
const MIN_BLOCK_SIZE: usize = 16;
const _: () = assert!(MIN_BLOCK_SIZE >= size_of::<FreeBlock>());

/// Represents a free block in the buddy allocator
struct FreeBlock {
    next: Option<NonNull<Self>>,
}

/// Buddy allocator for managing larger allocations
///
/// Uses a binary buddy system where blocks are split and merged in power-of-two sizes.
/// Each order maintains a free list of blocks of that size.
pub struct BuddyAllocator {
    /// Base pointer of the heap
    heap_start: *mut u8,
    /// Size of the heap in bytes
    heap_size: usize,
    /// Free lists for each order
    free_lists: [Option<NonNull<FreeBlock>>; MAX_ORDER],
    /// Maximum order supported
    max_order: usize,
}

impl BuddyAllocator {
    /// Create a new buddy allocator
    ///
    /// # Safety
    ///
    /// - `ptr` must be valid for reads and writes for `size` bytes
    /// - The memory region must not be used by other code while the allocator is active
    ///
    /// # Errors
    ///
    /// - `HeapError::InvalidSize` if the size is too small to initialize the allocator
    pub unsafe fn new(ptr: *mut u8, size: usize) -> Result<Self> {
        if size < MIN_BLOCK_SIZE {
            return Err(HeapError::InvalidSize);
        }

        // Align heap start to MIN_BLOCK_SIZE
        let addr = ptr.addr();
        let aligned_addr = align_up(addr, MIN_BLOCK_SIZE);
        let adjustment = aligned_addr - addr;

        if adjustment >= size {
            return Err(HeapError::InvalidSize);
        }

        let adjusted_size = size - adjustment;
        let aligned_ptr = ptr.with_addr(aligned_addr);

        let max_order = size_to_order(adjusted_size.next_power_of_two() / 2).min(MAX_ORDER - 1);

        let mut buddy = Self {
            heap_start: aligned_ptr,
            heap_size: adjusted_size,
            free_lists: [None; MAX_ORDER],
            max_order,
        };

        // Add the initial block to the appropriate free list
        let initial_order = size_to_order(adjusted_size.next_power_of_two() / 2);
        let initial_block_size = order_to_size(initial_order);

        if initial_block_size <= adjusted_size {
            // SAFETY: We've validated the heap region and aligned it properly
            unsafe {
                buddy.add_block_to_free_list(aligned_ptr, initial_order);
            }
        }

        Ok(buddy)
    }

    /// Allocate memory with the given layout
    ///
    /// # Errors
    ///
    /// - `HeapError::InvalidSize` if the requested size is zero
    /// - `HeapError::OutOfMemory` if there is not enough memory to satisfy the request
    pub fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>> {
        if layout.size() == 0 {
            return Err(HeapError::InvalidSize);
        }

        // Calculate required size including alignment
        let size = layout.size().max(MIN_BLOCK_SIZE);
        let align = layout.align().max(MIN_BLOCK_SIZE);

        // We need to allocate at least the size that satisfies both size and alignment
        let required_size = size.max(align).next_power_of_two();
        let order = size_to_order(required_size);

        if order > self.max_order {
            return Err(HeapError::OutOfMemory);
        }

        // Find a block of the appropriate order
        let block = self.find_block(order)?;

        // SAFETY: Block is guaranteed to be properly aligned and within heap bounds
        Ok(unsafe { NonNull::new_unchecked(block) })
    }

    /// Deallocate memory at the given pointer with the given layout
    ///
    /// # Safety
    ///
    /// - `ptr` must have been allocated by this allocator
    /// - `layout` must be the same as the layout used to allocate
    ///
    /// # Errors
    ///
    /// - `HeapError::InvalidPointer` if the pointer is out of bounds
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) -> Result<()> {
        let ptr_raw = ptr.as_ptr();

        // Verify pointer is within heap bounds
        let heap_end = unsafe { self.heap_start.add(self.heap_size) };
        if ptr_raw < self.heap_start || ptr_raw >= heap_end {
            return Err(HeapError::InvalidPointer);
        }

        let size = layout.size().max(MIN_BLOCK_SIZE);
        let align = layout.align().max(MIN_BLOCK_SIZE);
        let required_size = size.max(align).next_power_of_two();
        let order = size_to_order(required_size);

        // SAFETY: Caller guarantees this was a valid allocation
        unsafe {
            self.free_block(ptr_raw, order);
        }

        Ok(())
    }

    /// Find and allocate a block of the given order
    fn find_block(&mut self, order: usize) -> Result<*mut u8> {
        // Try to find a block in the current order
        if self.free_lists[order].is_some() {
            // SAFETY: Block comes from our free list
            return unsafe { Ok(self.pop_block_from_free_list(order)) };
        }

        // Try to split a larger block
        for higher_order in (order + 1)..=self.max_order {
            if self.free_lists[higher_order].is_some() {
                // Split blocks down to the required order
                for split_order in (order..higher_order).rev() {
                    // SAFETY: We know there's a block at split_order + 1
                    unsafe {
                        self.split_block(split_order + 1);
                    }
                }
                // SAFETY: After splitting, there's a block at the required order
                return unsafe { Ok(self.pop_block_from_free_list(order)) };
            }
        }

        Err(HeapError::OutOfMemory)
    }

    /// Split a block at the given order into two smaller blocks
    ///
    /// # Safety
    ///
    /// - There must be a free block at `order`
    unsafe fn split_block(&mut self, order: usize) {
        debug_assert!(order > 0);
        debug_assert!(order <= self.max_order);

        // SAFETY: Caller guarantees there's a block at this order
        let block_ptr = unsafe { self.pop_block_from_free_list(order) };
        let half_size = order_to_size(order) / 2;

        // Add both halves to the free list of order - 1
        // SAFETY: Both halves are within the original block's bounds
        unsafe {
            self.add_block_to_free_list(block_ptr, order - 1);
            let second_half = block_ptr.add(half_size);
            self.add_block_to_free_list(second_half, order - 1);
        }
    }

    /// Free a block and merge with its buddy if possible
    ///
    /// # Safety
    ///
    /// - `ptr` must be a valid block pointer that was previously allocated
    /// - `order` must be the correct order for this block
    unsafe fn free_block(&mut self, ptr: *mut u8, order: usize) {
        let mut current_ptr = ptr;
        let mut current_order = order;

        // Try to merge with buddy blocks
        while current_order < self.max_order {
            let buddy_ptr = self.buddy_of(current_ptr, current_order);

            // Check if buddy is free
            if self.is_block_free(buddy_ptr, current_order) {
                // Remove buddy from free list
                // SAFETY: We've verified the buddy is in the free list
                unsafe {
                    self.remove_block_from_free_list(buddy_ptr, current_order);
                }

                // Merge with buddy - the merged block starts at the lower address
                current_ptr = if current_ptr < buddy_ptr {
                    current_ptr
                } else {
                    buddy_ptr
                };
                current_order += 1;
            } else {
                break;
            }
        }

        // Add the (possibly merged) block to the free list
        // SAFETY: Block is valid and properly sized
        unsafe {
            self.add_block_to_free_list(current_ptr, current_order);
        }
    }

    /// Calculate the buddy of a block
    #[must_use]
    #[inline]
    const fn buddy_of(&self, ptr: *mut u8, order: usize) -> *mut u8 {
        let block_size = order_to_size(order);
        let offset = unsafe { ptr.offset_from(self.heap_start).cast_unsigned() };
        let buddy_offset = offset ^ block_size;
        unsafe { self.heap_start.add(buddy_offset) }
    }

    /// Check if a block is in the free list
    #[must_use]
    fn is_block_free(&self, ptr: *mut u8, order: usize) -> bool {
        let mut current = self.free_lists[order];

        while let Some(block) = current {
            if block.as_ptr().cast::<u8>() == ptr {
                return true;
            }
            // SAFETY: Block is in our free list
            current = unsafe { (*block.as_ptr()).next };
        }

        false
    }

    /// Add a block to the free list at the given order
    ///
    /// # Safety
    ///
    /// - `ptr` must point to valid memory within the heap
    /// - The block must not already be in any free list
    /// - The block must be properly sized for the given order
    unsafe fn add_block_to_free_list(&mut self, ptr: *mut u8, order: usize) {
        debug_assert!(order < MAX_ORDER);
        debug_assert!(ptr >= self.heap_start);
        debug_assert!(
            unsafe { ptr.add(order_to_size(order)) }
                <= unsafe { self.heap_start.add(self.heap_size) }
        );

        #[allow(clippy::cast_ptr_alignment)]
        let block = ptr.cast::<FreeBlock>();
        // SAFETY: Caller guarantees the memory is valid
        unsafe {
            (*block).next = self.free_lists[order];
        }
        // SAFETY: We just created a valid FreeBlock
        self.free_lists[order] = Some(unsafe { NonNull::new_unchecked(block) });
    }

    /// Remove and return a block from the free list at the given order
    ///
    /// # Safety
    ///
    /// - There must be at least one block in the free list at `order`
    unsafe fn pop_block_from_free_list(&mut self, order: usize) -> *mut u8 {
        debug_assert!(order < MAX_ORDER);
        debug_assert!(self.free_lists[order].is_some());

        let block = self.free_lists[order].unwrap();
        let block_ptr = block.as_ptr().cast::<u8>();

        // SAFETY: Caller guarantees block is in the free list
        self.free_lists[order] = unsafe { (*block.as_ptr()).next };

        block_ptr
    }

    /// Remove a specific block from the free list
    ///
    /// # Safety
    ///
    /// - The block at `ptr` must be in the free list at `order`
    unsafe fn remove_block_from_free_list(&mut self, ptr: *mut u8, order: usize) {
        debug_assert!(order < MAX_ORDER);

        let mut prev: Option<NonNull<FreeBlock>> = None;
        let mut current = self.free_lists[order];

        while let Some(block) = current {
            let block_ptr = block.as_ptr().cast::<u8>();

            if block_ptr == ptr {
                // Found the block to remove
                // SAFETY: Block is in the free list
                let next = unsafe { (*block.as_ptr()).next };

                if let Some(prev_block) = prev {
                    // SAFETY: prev_block is from our free list
                    unsafe {
                        (*prev_block.as_ptr()).next = next;
                    }
                } else {
                    // Removing the head of the list
                    self.free_lists[order] = next;
                }
                return;
            }

            prev = Some(block);
            // SAFETY: Block is in the free list
            current = unsafe { (*block.as_ptr()).next };
        }
    }
}

// SAFETY: Since the allocator manages a fixed memory region and doesn't share ownership,
// it's safe to send between threads and share across thread boundaries.
unsafe impl Send for BuddyAllocator {}
unsafe impl Sync for BuddyAllocator {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buddy_init() {
        let mut buffer = [0u8; 1024];
        let allocator = unsafe { BuddyAllocator::new(buffer.as_mut_ptr(), buffer.len()) };

        assert!(allocator.is_ok());
    }

    #[test]
    fn test_buddy_allocate_deallocate() {
        let mut buffer = [0u8; 4096];
        let mut allocator =
            unsafe { BuddyAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = allocator.allocate(layout);
        assert!(ptr.is_ok());

        unsafe {
            allocator.deallocate(ptr.unwrap(), layout).unwrap();
        }
    }

    #[test]
    fn test_buddy_multiple_allocations() {
        let mut buffer = [0u8; 8192];
        let mut allocator =
            unsafe { BuddyAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        let layout1 = Layout::from_size_align(128, 8).unwrap();
        let layout2 = Layout::from_size_align(256, 16).unwrap();
        let layout3 = Layout::from_size_align(64, 8).unwrap();

        let ptr1 = allocator.allocate(layout1).unwrap();
        let ptr2 = allocator.allocate(layout2).unwrap();
        let ptr3 = allocator.allocate(layout3).unwrap();

        // Verify they're different
        assert_ne!(ptr1.as_ptr(), ptr2.as_ptr());
        assert_ne!(ptr2.as_ptr(), ptr3.as_ptr());
        assert_ne!(ptr1.as_ptr(), ptr3.as_ptr());

        unsafe {
            allocator.deallocate(ptr1, layout1).unwrap();
            allocator.deallocate(ptr2, layout2).unwrap();
            allocator.deallocate(ptr3, layout3).unwrap();
        }
    }

    #[test]
    fn test_buddy_out_of_memory() {
        let mut buffer = [0u8; 512];
        let mut allocator =
            unsafe { BuddyAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

        // Try to allocate more than available
        let layout = Layout::from_size_align(1024, 8).unwrap();
        let result = allocator.allocate(layout);
        assert!(matches!(result, Err(HeapError::OutOfMemory)));
    }
}
