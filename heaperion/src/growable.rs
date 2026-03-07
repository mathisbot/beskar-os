//! Growable heap wrapper
//!
//! Provides [`GrowableHeap`], a wrapper around any heaps that
//! automatically requests additional memory from a [`MemorySource`] when the
//! current regions are exhausted.
use crate::error::{HeapError, Result};
use core::alloc::Layout;
use core::ptr::NonNull;

const MINIMAL_GROW_SIZE: usize = 1024 * 1024; // 1 MiB

pub type DefaultGrowableHeap<S, const MAX_REGIONS: usize = 8> =
    GrowableHeap<crate::HybridAllocator, crate::BuddyAllocator, S, MAX_REGIONS>;

/// A provider of additional memory regions for a [`GrowableHeap`].
///
/// # Safety
///
/// Every region returned by [`MemorySource::request`] must satisfy the following contract:
///
/// - valid for reads and writes for `size` bytes,
/// - not aliased by any other live allocation,
/// - alive for as long as the [`GrowableHeap`] that uses it.
pub unsafe trait MemorySource {
    /// Request a new memory region of at least `min_size` bytes.
    ///
    /// Returns `Some((ptr, size))` on success where `size >= min_size`, or
    /// `None` if no more memory can be provided.
    fn request(&mut self, min_size: usize) -> Option<(*mut u8, usize)>;
}

/// A Heap backend suitable for use with [`GrowableHeap`].
pub trait HeapBackend {
    /// Allocate memory with the given layout.
    ///
    /// # Errors
    ///
    /// Returns an error if the allocation cannot be satisfied.
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>>;

    /// Deallocate memory at the given pointer.
    ///
    /// # Safety
    ///
    /// - `ptr` must have been allocated by this backend instance.
    /// - `layout` must match the layout used during allocation.
    ///
    /// # Errors
    ///
    /// Returns an error if `ptr` is not owned by this backend.
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) -> Result<()>;

    /// Returns true if this backend claims ownership of the given pointer.
    fn contains(&self, ptr: NonNull<u8>) -> bool;
}
/// A Heap backend that can be used as an additional region in a [`GrowableHeap`].
pub trait AdditionalHeapBackend: HeapBackend + Sized {
    /// Create a new heap backend instance over the given memory region.
    ///
    /// # Safety
    ///
    /// - `ptr` must be valid for reads and writes for `size` bytes.
    /// - The region must not be aliased by any other allocation.
    /// - The region must remain valid for the lifetime of the returned instance.
    ///
    /// # Errors
    ///
    /// Returns an error if `size` is too small to initialise the backend.
    unsafe fn new(ptr: *mut u8, size: usize) -> Result<Self>;
}

/// A heap wrapper that automatically grows when memory is exhausted.
///
/// When an allocation fails across all current regions, [`GrowableHeap`] calls
/// [`MemorySource::request`], initialises a fresh [`AdditionalHeapBackend`] over the
/// returned region, and retries.  If the source cannot satisfy the request, or
/// `MAX_REGIONS` has already been reached, [`HeapError::OutOfMemory`] is
/// returned to the caller.
///
/// # Example
///
/// ```rust
/// # use heaperion::{BuddyAllocator, GrowableHeap, MemorySource, HybridAllocator};
/// # extern crate alloc;
/// # use alloc::vec::Vec;
/// #
/// struct VecSource {
///     pool: Vec<u8>,
///     offset: usize,
/// }
///
/// unsafe impl MemorySource for VecSource {
///     fn request(&mut self, min_size: usize) -> Option<(*mut u8, usize)> {
///         let remaining = self.pool.len() - self.offset;
///         if remaining < min_size {
///             return None;
///         }
///         let ptr = unsafe { self.pool.as_mut_ptr().add(self.offset) };
///         self.offset += remaining;
///         Some((ptr, remaining))
///     }
/// }
///
/// let mut buffer = [0u8; 65536];
/// let initial = unsafe { HybridAllocator::new(buffer.as_mut_ptr(), buffer.len()) }.unwrap();
/// let pool = alloc::vec![0u8; 1 << 20]; // 1 MiB fallback pool
/// let source = VecSource { pool, offset: 0 };
/// let mut heap: GrowableHeap<HybridAllocator, BuddyAllocator, _, 4> =
///     GrowableHeap::new(initial, source);
/// ```
pub struct GrowableHeap<
    B: HeapBackend,
    A: AdditionalHeapBackend,
    S: MemorySource,
    const MAX_REGIONS: usize = 8,
> {
    /// Initial allocator region.
    base: B,
    /// Fixed-size array; `additional[0..count]` are always `Some`.
    additional: [Option<A>; MAX_REGIONS],
    /// Number of active regions.
    count: usize,
    /// Source of additional memory.
    source: S,
}

impl<B, A, S, const MAX_REGIONS: usize> GrowableHeap<B, A, S, MAX_REGIONS>
where
    B: HeapBackend,
    A: AdditionalHeapBackend,
    S: MemorySource,
{
    /// Create a growable heap from an already-initialised [`HybridAllocator`] and a
    /// [`MemorySource`].
    pub const fn new(base: B, source: S) -> Self {
        Self {
            base,
            additional: [const { None }; MAX_REGIONS],
            count: 0,
            source,
        }
    }

    /// Allocate memory with the given layout.
    ///
    /// Tries each existing region in order. If all fail, requests a new region
    /// of at least `layout.size()` bytes via the [`MemorySource`] and retries.
    ///
    /// # Errors
    ///
    /// - [`HeapError::OutOfMemory`] if all current regions are full and
    ///   either `MAX_REGIONS` is reached or the source returns `None`.
    ///
    /// # Panics
    ///
    /// Panics if an internal invariant is violated (newly grown region slot is empty).
    /// This cannot happen under normal operation.
    pub fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>> {
        // Fast path: try the base region first.
        if let Ok(ptr) = self.base.allocate(layout) {
            return Ok(ptr);
        }
        // Then try the additional regions in order.
        for region in self.additional[..self.count].iter_mut().flatten() {
            if let Ok(ptr) = region.allocate(layout) {
                return Ok(ptr);
            }
        }

        // Slow path: request a new region and retry once.
        self.grow(layout.size())?;
        self.additional[self.count - 1]
            .as_mut()
            .expect("region was just added")
            .allocate(layout)
            .map_err(|_| HeapError::OutOfMemory)
    }

    /// Deallocate memory at the given pointer.
    ///
    /// # Safety
    ///
    /// - `ptr` must have been allocated by this [`GrowableHeap`] instance.
    /// - `layout` must match the layout used during allocation.
    ///
    /// # Errors
    ///
    /// - [`HeapError::InvalidPointer`] if no region claims ownership of `ptr`.
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) -> Result<()> {
        if self.base.contains(ptr) {
            // SAFETY: `contains` verified ownership; forwarded from caller.
            return unsafe { self.base.deallocate(ptr, layout) };
        }
        for region in self.additional[..self.count].iter_mut().flatten() {
            if region.contains(ptr) {
                // SAFETY: `contains` verified ownership; forwarded from caller.
                return unsafe { region.deallocate(ptr, layout) };
            }
        }
        Err(HeapError::InvalidPointer)
    }

    /// Returns the number of memory regions currently active (initial + grows).
    #[must_use]
    #[inline]
    pub const fn region_count(&self) -> usize {
        self.count + 1 // +1 for the base region
    }

    /// Returns the compile-time maximum number of regions this heap can hold.
    #[must_use]
    #[inline]
    pub const fn max_regions() -> usize {
        MAX_REGIONS + 1 // +1 for the base region
    }

    #[must_use]
    #[inline]
    const fn grow_strategy(min_size: usize) -> usize {
        // Note: On small allocations, `min_size.max(MINIMAL_GROW_SIZE)` is approximately
        // `min_size + MINIMAL_GROW_SIZE` and on large allocations it is approximately `min_size`.
        // This still ensures that memory overhead from the underlying allocator is amortised.
        min_size.next_power_of_two() + MINIMAL_GROW_SIZE
    }

    /// Request a new region from the source, construct a heap over it, and
    /// append it to the regions array.
    fn grow(&mut self, min_size: usize) -> Result<()> {
        if self.count >= MAX_REGIONS {
            return Err(HeapError::OutOfMemory);
        }

        let (ptr, size) = self
            .source
            .request(Self::grow_strategy(min_size))
            .ok_or(HeapError::OutOfMemory)?;

        // SAFETY: MemorySource contract guarantees the region is valid and
        // exclusively owned for the lifetime of this GrowableHeap.
        let heap = unsafe { A::new(ptr, size) }.map_err(|_| HeapError::OutOfMemory)?;

        self.additional[self.count] = Some(heap);
        self.count += 1;

        Ok(())
    }
}

// SAFETY: GrowableHeap is Send+Sync if S is, which is enforced by the
// where bounds below.  Raw pointers inside H are already accounted for by H's
// own Send/Sync impls.
unsafe impl<B: HeapBackend + Send, A: AdditionalHeapBackend + Send, S, const N: usize> Send
    for GrowableHeap<B, A, S, N>
where
    S: MemorySource + Send,
{
}
unsafe impl<B: HeapBackend + Sync, A: AdditionalHeapBackend + Sync, S, const N: usize> Sync
    for GrowableHeap<B, A, S, N>
where
    S: MemorySource + Sync,
{
}

mod impls {
    use super::{AdditionalHeapBackend, HeapBackend};
    use crate::{BuddyAllocator, HybridAllocator, Result, SlabAllocator};

    impl HeapBackend for HybridAllocator {
        fn allocate(&mut self, layout: core::alloc::Layout) -> Result<core::ptr::NonNull<u8>> {
            self.allocate(layout)
        }

        unsafe fn deallocate(
            &mut self,
            ptr: core::ptr::NonNull<u8>,
            layout: core::alloc::Layout,
        ) -> Result<()> {
            unsafe { self.deallocate(ptr, layout) }
        }

        fn contains(&self, ptr: core::ptr::NonNull<u8>) -> bool {
            self.contains(ptr)
        }
    }
    impl AdditionalHeapBackend for HybridAllocator {
        unsafe fn new(ptr: *mut u8, size: usize) -> Result<Self> {
            unsafe { Self::new(ptr, size) }
        }
    }

    impl HeapBackend for BuddyAllocator {
        fn allocate(&mut self, layout: core::alloc::Layout) -> Result<core::ptr::NonNull<u8>> {
            self.allocate(layout)
        }

        unsafe fn deallocate(
            &mut self,
            ptr: core::ptr::NonNull<u8>,
            layout: core::alloc::Layout,
        ) -> Result<()> {
            unsafe { self.deallocate(ptr, layout) }
        }

        fn contains(&self, ptr: core::ptr::NonNull<u8>) -> bool {
            self.contains(ptr)
        }
    }
    impl AdditionalHeapBackend for BuddyAllocator {
        unsafe fn new(ptr: *mut u8, size: usize) -> Result<Self> {
            unsafe { Self::new(ptr, size) }
        }
    }

    impl HeapBackend for SlabAllocator {
        fn allocate(&mut self, layout: core::alloc::Layout) -> Result<core::ptr::NonNull<u8>> {
            self.allocate(layout)
        }

        unsafe fn deallocate(
            &mut self,
            ptr: core::ptr::NonNull<u8>,
            _layout: core::alloc::Layout,
        ) -> Result<()> {
            unsafe { self.deallocate(ptr) }
        }

        fn contains(&self, ptr: core::ptr::NonNull<u8>) -> bool {
            self.contains(ptr)
        }
    }
    impl AdditionalHeapBackend for SlabAllocator {
        unsafe fn new(ptr: *mut u8, size: usize) -> Result<Self> {
            unsafe { Self::new(ptr, size) }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{BuddyAllocator, HybridAllocator};

    use super::*;

    extern crate alloc;
    use alloc::vec;
    use alloc::vec::Vec;

    /// A trivial `MemorySource` backed by a static slice of pre-allocated buffers.
    struct FixedSource<'a> {
        chunks: &'a mut [Option<alloc::vec::Vec<u8>>],
        idx: usize,
    }

    impl<'a> FixedSource<'a> {
        fn new(chunks: &'a mut [Option<alloc::vec::Vec<u8>>]) -> Self {
            Self { chunks, idx: 0 }
        }
    }

    // SAFETY: memory is valid, exclusively owned, and outlives the test.
    unsafe impl MemorySource for FixedSource<'_> {
        fn request(&mut self, _min_size: usize) -> Option<(*mut u8, usize)> {
            while self.idx < self.chunks.len() {
                let i = self.idx;
                self.idx += 1;
                if let Some(buf) = &mut self.chunks[i] {
                    let ptr = buf.as_mut_ptr();
                    let len = buf.len();
                    return Some((ptr, len));
                }
            }
            None
        }
    }

    #[test]
    fn test_growable_basic_allocation() {
        let mut initial_buf = vec![0u8; 16_384];
        let initial =
            unsafe { HybridAllocator::new(initial_buf.as_mut_ptr(), initial_buf.len()) }.unwrap();

        let source = FixedSource::new(&mut []);
        let mut heap: GrowableHeap<_, BuddyAllocator, _, 4> = GrowableHeap::new(initial, source);

        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = heap.allocate(layout).unwrap();
        assert_eq!(heap.region_count(), 1);
        unsafe { heap.deallocate(ptr, layout).unwrap() };
    }

    #[test]
    fn test_growable_triggers_growth() {
        // A very small initial heap that will exhaust quickly.
        let mut initial_buf = vec![0u8; 512];
        let initial =
            unsafe { BuddyAllocator::new(initial_buf.as_mut_ptr(), initial_buf.len()) }.unwrap();

        let mut extra_buf = Some(vec![0u8; 65_536]);
        let chunks = core::array::from_mut(&mut extra_buf);
        let source = FixedSource::new(chunks);

        let mut heap: GrowableHeap<_, BuddyAllocator, _, 4> = GrowableHeap::new(initial, source);

        // Exhaust the initial region with a large allocation.
        let big_layout = Layout::from_size_align(1024, 8).unwrap();
        let ptr = heap.allocate(big_layout).unwrap();

        // A second region must have been created.
        assert_eq!(heap.region_count(), 2);

        unsafe { heap.deallocate(ptr, big_layout).unwrap() };
    }

    #[test]
    fn test_growable_oom_no_source() {
        let mut initial_buf = vec![0u8; 64];
        let initial =
            unsafe { BuddyAllocator::new(initial_buf.as_mut_ptr(), initial_buf.len()) }.unwrap();

        let source = FixedSource::new(&mut []);
        let mut heap: GrowableHeap<_, BuddyAllocator, _, 4> = GrowableHeap::new(initial, source);

        // Should fail – no extra memory available.
        let huge_layout = Layout::from_size_align(8192, 8).unwrap();
        assert!(matches!(
            heap.allocate(huge_layout),
            Err(HeapError::OutOfMemory)
        ));
    }

    #[test]
    fn test_growable_deallocate_across_regions() {
        // Allocate from both the initial region and a grown one; free in reverse order.
        let mut initial_buf = vec![0u8; 512];
        let initial =
            unsafe { BuddyAllocator::new(initial_buf.as_mut_ptr(), initial_buf.len()) }.unwrap();

        let mut extra_buf = Some(vec![0u8; 65_536]);
        let chunks = core::array::from_mut(&mut extra_buf);
        let source = FixedSource::new(chunks);

        let mut heap: GrowableHeap<_, BuddyAllocator, _, 4> = GrowableHeap::new(initial, source);

        let small_layout = Layout::from_size_align(16, 8).unwrap();
        let large_layout = Layout::from_size_align(1024, 8).unwrap();

        // This fits in region 0.
        let ptr_small = heap.allocate(small_layout).unwrap();
        // This forces a grow into region 1.
        let ptr_large = heap.allocate(large_layout).unwrap();

        assert_eq!(heap.region_count(), 2);

        unsafe {
            heap.deallocate(ptr_large, large_layout).unwrap();
            heap.deallocate(ptr_small, small_layout).unwrap();
        }
    }

    #[test]
    fn test_growable_multiple_grows() {
        let mut initial_buf = vec![0u8; 128];
        let initial =
            unsafe { BuddyAllocator::new(initial_buf.as_mut_ptr(), initial_buf.len()) }.unwrap();

        let mut extras: [Option<Vec<u8>>; 3] = [
            Some(vec![0u8; 256]),
            Some(vec![0u8; 512]),
            Some(vec![0u8; 1_024]),
        ];
        let source = FixedSource::new(&mut extras);

        let mut heap: GrowableHeap<_, BuddyAllocator, _, 4> = GrowableHeap::new(initial, source);
        let mut ptrs = Vec::new();

        for _ in 0..4 {
            let layout = Layout::from_size_align(128, 8).unwrap();
            if let Ok(ptr) = heap.allocate(layout) {
                ptrs.push((ptr, layout));
            }
        }

        assert!(
            heap.region_count() > 1,
            "should have grown at least once; count={}",
            heap.region_count()
        );

        for (ptr, layout) in ptrs {
            unsafe { heap.deallocate(ptr, layout).unwrap() };
        }
    }
}
