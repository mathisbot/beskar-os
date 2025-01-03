use core::{mem::MaybeUninit, sync::atomic::AtomicU8};

use crate::cpu::cpuid;
use core::sync::atomic::Ordering;

/// "Safe" wrapper around the RDRAND instruction
fn rdrand(dst: &mut u64) {
    static IS_SUPPORTED: AtomicU8 = AtomicU8::new(2); // 2 = Uninitialized, 1 = Supported, 0 = Not Supported

    #[cold]
    fn check_support() -> bool {
        let rdrand_supported = cpuid::check_feature(cpuid::CpuFeature::RDRAND);
        IS_SUPPORTED.store(u8::from(rdrand_supported), Ordering::Relaxed);
        rdrand_supported
    }

    let is_supported = match IS_SUPPORTED.load(Ordering::Acquire) {
        0 => false,
        1 => true,
        2 => check_support(),
        _ => unreachable!(),
    };

    assert!(is_supported, "RDRAND not supported");

    let res = unsafe { core::arch::x86_64::_rdrand64_step(dst) };
    assert_eq!(res, 1, "RDRAND failed");
}

#[must_use]
/// Generates a random instance of the given type, filling its bytes with RDRAND
///
/// ## Safety
///
/// Every random sequence of bits must be a valid instance of the given type
pub unsafe fn rand<T: Sized>() -> T {
    let mut res = MaybeUninit::<T>::uninit();

    // Safety:
    // `MaybeUninit` guarantees that the layout is the same as `T`
    // so that the memory is valid for writes.
    let slice =
        unsafe { core::slice::from_raw_parts_mut(res.as_mut_ptr().cast::<u8>(), size_of::<T>()) };

    rand_bytes(slice);

    // Safety:
    // We just initialized the value and because of the function's safety guards,
    // the result is a valid instance of the given type
    unsafe { res.assume_init() }
}

/// Randomly fills memory with RDRAND.
///
/// This function is a shortcut for `core::slice::from_raw_parts_mut`
/// followed by `rand_bytes`.
///
/// ## Safety
///
/// Every random sequence of bits must be a valid instance of the given type
/// and the given pointer and length must be valid
///
/// ## Example
///
/// Please note that in this example, it may be safer to use `rand_bytes` directly.
/// This function could be used for complex structs that are not `Sized`.
///
/// ```rust,no_run
/// # use kernel::cpu::rand::rand_unsized;
/// # extern crate alloc;
/// # use alloc::vec::Vec;
/// #
/// let mut buffer = vec![0_u32; 256];
///
/// unsafe {
///     rand_unsized(buffer.as_mut_ptr(), buffer.len());
/// }
/// ```
pub unsafe fn rand_unsized<T>(ptr: *mut T, len: usize) {
    let slice = unsafe { core::slice::from_raw_parts_mut(ptr.cast(), len * size_of::<T>()) };

    rand_bytes(slice);
}

/// Generates random bytes using RDRAND
pub fn rand_bytes(bytes: &mut [u8]) {
    // It is safe to cast 8 `u8`s to a `u64`
    let (start_u8, middle_u64, end_u8) = unsafe { bytes.align_to_mut::<u64>() };

    // Quickly fill the middle part of the slice with random u64s
    for qword in middle_u64 {
        rdrand(qword);
    }

    // Now fill the unaligned start and end of the slice
    let mut randomized = 0;

    if !start_u8.is_empty() {
        // One call to `rdrand` is enough as `start_u8.len()` is less than 8
        rdrand(&mut randomized);
        start_u8.copy_from_slice(&randomized.to_ne_bytes()[..start_u8.len()]);
    }

    if !end_u8.is_empty() {
        // One call to `rdrand` is enough as `end_u8.len()` is less than 8
        rdrand(&mut randomized);
        end_u8.copy_from_slice(&randomized.to_ne_bytes()[..end_u8.len()]);
    }
}
