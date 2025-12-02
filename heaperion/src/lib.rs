//! # Heaperion: High-Performance `no_std` Heap Allocator
//!
//! Heaperion provides a robust, efficient, and safe heap allocator for `no_std` environments.
//! It combines multiple allocation strategies to provide optimal performance across different
//! allocation sizes and patterns.
//!
//! ## Architecture
//!
//! The allocator uses a hybrid approach:
//! - **Slab Allocator**: For small, frequently allocated objects (<= 512 bytes)
//!   - O(1) allocation and deallocation
//!   - Excellent cache locality
//!   - Minimal fragmentation for fixed-size allocations
//!
//! - **Buddy Allocator**: For larger allocations (> 512 bytes)
//!   - O(log(n)) allocation and deallocation
//!   - Power-of-two sized blocks
//!   - Automatic coalescing to reduce fragmentation
//!
//! ## Usage
//!
//! ### Basic Usage
//!
//! ```rust
//! use heaperion::HybridAllocator;
//! use core::alloc::Layout;
//!
//! // Create a memory buffer for the heap
//! let mut buffer = [0u8; 65536];
//!
//! let mut allocator =
//!     unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();
//!
//! // Allocate memory
//! let layout = Layout::from_size_align(128, 8).unwrap();
//! let ptr = allocator.allocate(layout).unwrap();
//!
//! // Use the memory...
//!
//! // Deallocate when done
//! unsafe {
//!     allocator.deallocate(ptr, layout).unwrap();
//! }
//! ```
#![warn(clippy::pedantic, clippy::nursery)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![no_std]

mod buddy;
mod error;
mod hybrid;
mod slab;
mod utils;

// Public exports
pub use buddy::BuddyAllocator;
pub use error::{HeapError, Result};
pub use hybrid::HybridAllocator;
pub use slab::SlabAllocator;

/// The main heap allocator type
///
/// This is an alias to `HybridAllocator` for convenience.
pub type Heap = HybridAllocator;
