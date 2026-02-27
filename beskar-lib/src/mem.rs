use crate::error::{MemoryError, MemoryErrorKind, MemoryResult};
use beskar_core::arch::paging::{M4KiB, MemSize as _};
use core::{num::NonZeroU64, ptr::NonNull};
use heaperion::{DefaultGrowableHeap, HybridAllocator};
use hyperdrive::locks::mcs::MUMcsLock;

static ALLOCATOR: MUMcsLock<DefaultGrowableHeap<Mmap, 4>> = MUMcsLock::uninit();

#[global_allocator]
static HEAP: Heap = Heap;

const HEAP_START_SIZE: u64 = 16 * 1024 * 1024; // 16 MiB
beskar_core::static_assert!(HEAP_START_SIZE.is_multiple_of(M4KiB::SIZE));

struct Heap;

unsafe impl core::alloc::GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let Some(Ok(res)) = ALLOCATOR.with_locked_if_init(|heap| heap.allocate(layout)) else {
            return core::ptr::null_mut();
        };
        res.as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        ALLOCATOR.with_locked_if_init(|heap| unsafe {
            let _ = heap.deallocate(NonNull::new_unchecked(ptr), layout);
        });
    }
}

#[inline]
/// Initialize the heap allocator
pub(super) fn init_heap() {
    let size = HEAP_START_SIZE;
    let start = mmap(size, None, MemoryProtection::ReadWrite).unwrap();

    // SAFETY: `start` and `size` come from a successful call to `mmap` and are not used after this point.
    let heap =
        unsafe { HybridAllocator::new(start.as_ptr(), usize::try_from(size).unwrap()) }.unwrap();
    let growable = DefaultGrowableHeap::new(heap, Mmap);
    ALLOCATOR.init(growable);
}

/// Map memory into the address space
///
/// # Errors
///
/// Returns an error if the memory cannot be mapped.
pub fn mmap(
    size: u64,
    alignment: Option<NonZeroU64>,
    flags: MemoryProtection,
) -> MemoryResult<NonNull<u8>> {
    if alignment.is_some_and(|a| !a.get().is_power_of_two()) {
        return Err(MemoryError::new(MemoryErrorKind::InvalidAlignment));
    }

    let ptr = crate::sys::sc_mmap(size, alignment.map_or(1, NonZeroU64::get), flags as _);

    NonNull::new(ptr).ok_or_else(|| MemoryError::new(MemoryErrorKind::OutOfMemory))
}

/// Unmap memory from the address space
///
/// Returns true if the operation was successful, false otherwise.
///
/// # Safety
///
/// The pointer and size must be valid and correspond to a previously mapped region
/// that will no longer be used after this call.
pub unsafe fn munmap(ptr: *mut u8, size: u64) -> bool {
    let res = crate::sys::sc_munmap(ptr, size);
    res.is_success()
}

/// Change the protection of a memory region
///
/// Returns true if the operation was successful, false otherwise.
///
/// Note that the pointer and size must be page-aligned.
pub fn mprotect(ptr: *mut u8, size: u64, flags: MemoryProtection) -> bool {
    let res = crate::sys::sc_mprotect(ptr, size, flags as _);
    res.is_success()
}

pub struct MmapReadWrite {
    ptr: NonNull<u8>,
    size: u64,
}

impl MmapReadWrite {
    #[inline]
    /// Create a new read-write memory mapping of the given size.
    ///
    /// # Errors
    ///
    /// Returns an error if the memory cannot be mapped.
    pub fn new(size: u64) -> MemoryResult<Self> {
        let ptr = mmap(size, None, MemoryProtection::ReadWrite)?;
        Ok(Self { ptr, size })
    }

    #[must_use]
    #[inline]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    #[inline]
    pub const fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    pub fn as_slice(&self) -> &[u8] {
        let data = self.as_ptr();
        let len = usize::try_from(self.size).unwrap();
        unsafe { core::slice::from_raw_parts(data.cast::<u8>(), len) }
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        let data = self.as_ptr();
        let len = usize::try_from(self.size).unwrap();
        unsafe { core::slice::from_raw_parts_mut(data.cast::<u8>(), len) }
    }
}

impl Drop for MmapReadWrite {
    fn drop(&mut self) {
        // Safety: `ptr` and `size` come from a previous, successful call to `mmap`
        // and are not used after this point.
        unsafe {
            munmap(self.ptr.as_ptr(), self.size);
        }
    }
}

struct Mmap;
unsafe impl heaperion::MemorySource for Mmap {
    fn request(&mut self, min_size: usize) -> Option<(*mut u8, usize)> {
        let ptr = mmap(
            u64::try_from(min_size).unwrap(),
            None,
            MemoryProtection::ReadWrite,
        )
        .ok()?;
        Some((ptr.as_ptr(), min_size))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u64)]
pub enum MemoryProtection {
    NoAccess = 0,
    ReadOnly = beskar_core::syscall::consts::MFLAGS_READ,
    ReadWrite =
        beskar_core::syscall::consts::MFLAGS_READ | beskar_core::syscall::consts::MFLAGS_WRITE,
    ReadExecute =
        beskar_core::syscall::consts::MFLAGS_READ | beskar_core::syscall::consts::MFLAGS_EXECUTE,
    ReadWriteExecute = beskar_core::syscall::consts::MFLAGS_READ
        | beskar_core::syscall::consts::MFLAGS_WRITE
        | beskar_core::syscall::consts::MFLAGS_EXECUTE,
}
