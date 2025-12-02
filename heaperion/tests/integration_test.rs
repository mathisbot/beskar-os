//! Integration tests for the heaperion allocator

extern crate alloc;
use alloc::vec::Vec;

use core::alloc::Layout;
use heaperion::HybridAllocator;

#[test]
fn test_basic_allocation_flow() {
    let mut buffer = [0u8; 65536];
    let mut allocator = unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

    // Test various sizes
    let sizes = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
    let mut allocations = Vec::new();

    for &size in &sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        match allocator.allocate(layout) {
            Ok(ptr) => allocations.push((ptr, layout)),
            Err(_) => break,
        }
    }

    // Verify we got some allocations
    assert!(!allocations.is_empty());

    // Deallocate all
    for (ptr, layout) in allocations {
        unsafe {
            allocator.deallocate(ptr, layout).unwrap();
        }
    }
}

#[test]
fn test_allocation_patterns() {
    let mut buffer = [0u8; 131072];
    let mut allocator = unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

    // Pattern 1: Allocate many small blocks
    let mut small_blocks = Vec::new();
    for _ in 0..50 {
        let layout = Layout::from_size_align(32, 8).unwrap();
        if let Ok(ptr) = allocator.allocate(layout) {
            small_blocks.push((ptr, layout));
        }
    }
    assert!(small_blocks.len() > 10);

    // Pattern 2: Allocate a few large blocks
    let mut large_blocks = Vec::new();
    for _ in 0..5 {
        let layout = Layout::from_size_align(4096, 16).unwrap();
        if let Ok(ptr) = allocator.allocate(layout) {
            large_blocks.push((ptr, layout));
        }
    }

    // Pattern 3: Free half the small blocks
    for (ptr, layout) in small_blocks.iter().step_by(2) {
        unsafe {
            allocator.deallocate(*ptr, *layout).unwrap();
        }
    }

    // Pattern 4: Allocate more small blocks (should reuse freed space)
    for _ in 0..10 {
        let layout = Layout::from_size_align(32, 8).unwrap();
        let _ = allocator.allocate(layout);
    }

    // Cleanup
    for (ptr, layout) in small_blocks.iter().skip(1).step_by(2) {
        unsafe {
            allocator.deallocate(*ptr, *layout).unwrap();
        }
    }
    for (ptr, layout) in large_blocks {
        unsafe {
            allocator.deallocate(ptr, layout).unwrap();
        }
    }
}

#[test]
fn test_alignment_requirements() {
    let mut buffer = [0u8; 65536];
    let mut allocator = unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

    // Test various alignment requirements
    let alignments = [8, 16, 32, 64, 128];

    for &align in &alignments {
        let layout = Layout::from_size_align(64, align).unwrap();
        if let Ok(ptr) = allocator.allocate(layout) {
            // Verify alignment
            let addr = ptr.as_ptr().addr();
            assert_eq!(addr % align, 0, "Allocation not aligned to {}", align);

            unsafe {
                allocator.deallocate(ptr, layout).unwrap();
            }
        }
    }
}

#[test]
fn test_stress_mixed_operations() {
    let mut buffer = [0u8; 262144];
    let mut allocator = unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

    let mut active_allocations = Vec::new();
    let mut rng_state = 12345u32;

    // Simple LCG for deterministic testing
    let mut next_random = || {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        rng_state
    };

    // Perform 100 random operations
    for _ in 0..100 {
        let op = next_random() % 100;

        if op < 60 && active_allocations.len() < 50 {
            // 60% chance to allocate
            let size_class = next_random() % 5;
            let size = match size_class {
                0 => 16,
                1 => 64,
                2 => 256,
                3 => 1024,
                _ => 4096,
            };

            let layout = Layout::from_size_align(size, 8).unwrap();
            if let Ok(ptr) = allocator.allocate(layout) {
                active_allocations.push((ptr, layout));
            }
        } else if !active_allocations.is_empty() {
            // 40% chance to deallocate (if we have allocations)
            let index = usize::try_from(next_random()).unwrap() % active_allocations.len();
            let (ptr, layout) = active_allocations.swap_remove(index);
            unsafe {
                allocator.deallocate(ptr, layout).unwrap();
            }
        }
    }

    // Cleanup remaining allocations
    for (ptr, layout) in active_allocations {
        unsafe {
            allocator.deallocate(ptr, layout).unwrap();
        }
    }
}

#[test]
fn test_fragmentation_handling() {
    let mut buffer = [0u8; 131072];
    let mut allocator = unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

    // Create fragmentation by allocating and freeing alternating blocks
    let layout_small = Layout::from_size_align(128, 8).unwrap();
    let layout_large = Layout::from_size_align(1024, 8).unwrap();

    let mut small_blocks = Vec::new();
    let mut large_blocks = Vec::new();

    // Allocate alternating small and large blocks
    for _i in 0..10 {
        if let Ok(ptr) = allocator.allocate(layout_small) {
            small_blocks.push(ptr);
        }
        if let Ok(ptr) = allocator.allocate(layout_large) {
            large_blocks.push(ptr);
        }
    }

    // Free all large blocks (creates gaps)
    for ptr in large_blocks {
        unsafe {
            allocator.deallocate(ptr, layout_large).unwrap();
        }
    }

    // Try to allocate large blocks again (should reuse freed space)
    let mut new_large_blocks = Vec::new();
    for _ in 0..5 {
        if let Ok(ptr) = allocator.allocate(layout_large) {
            new_large_blocks.push(ptr);
        }
    }

    // Should have successfully allocated some blocks
    assert!(!new_large_blocks.is_empty());

    // Cleanup
    for ptr in small_blocks {
        unsafe {
            allocator.deallocate(ptr, layout_small).unwrap();
        }
    }
    for ptr in new_large_blocks {
        unsafe {
            allocator.deallocate(ptr, layout_large).unwrap();
        }
    }
}

#[test]
fn test_boundary_sizes() {
    let mut buffer = [0u8; 65536];
    let mut allocator = unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();

    // Test boundary sizes between slab and buddy
    let sizes = [
        1, 8, 15, 16, 32, 63, 64, 127, 128, 255, 256, 511, 512, 1023, 1024, 2048,
    ];

    for &size in &sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        if let Ok(ptr) = allocator.allocate(layout) {
            unsafe {
                allocator.deallocate(ptr, layout).unwrap();
            }
        }
    }
}
