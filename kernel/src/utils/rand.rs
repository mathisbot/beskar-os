#![allow(dead_code)]

use core::sync::atomic::AtomicU8;

use crate::cpu::cpuid;

/// "Safe" wrapper around the RDRAND instruction
fn rdrand(dst: &mut u64) {
    static IS_SUPPORTED: AtomicU8 = AtomicU8::new(2); // 2 = Uninitialized

    #[cold]
    fn check_support() -> bool {
        let rdrand_supported = cpuid::check_feature(cpuid::CpuFeature::RDRAND);
        // We could use compare_exchange here, but it is not a problem to set the value multiple times
        // as it won't change (support for RDRAND won't magically appear or disappear)
        IS_SUPPORTED.store(
            u8::from(rdrand_supported),
            core::sync::atomic::Ordering::Relaxed,
        );
        rdrand_supported
    }

    let is_supported = {
        let val = IS_SUPPORTED.load(core::sync::atomic::Ordering::Relaxed);
        if val == 2 {
            check_support()
        } else {
            val == 1
        }
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
/// Every random sequence must be a valid instance of the given type
pub unsafe fn rand<T: Sized>() -> T {
    let mut res = core::mem::MaybeUninit::<T>::uninit();

    let total_size = size_of::<T>();
    let full_rounds = total_size / size_of::<u64>();
    let remaining_bytes = total_size % size_of::<u64>();

    if full_rounds > 0 {
        // Safety:
        // The size of the slice is smaller than the size of the given type
        let dst_slice =
            core::slice::from_raw_parts_mut(res.as_mut_ptr().cast::<u64>(), full_rounds);

        for u in dst_slice {
            rdrand(u);
        }
    }

    if remaining_bytes > 0 {
        let remaining_slice = core::slice::from_raw_parts_mut(
            res.as_mut_ptr()
                .cast::<u8>()
                .add(total_size - remaining_bytes),
            remaining_bytes,
        );
        let mut randomized = 0;
        rdrand(&mut randomized);
        remaining_slice.copy_from_slice(&randomized.to_ne_bytes()[..remaining_bytes]);
    }

    // Safety:
    // We just initialized the value and because of the function's safety guards,
    // the result is a valid instance of the given type
    unsafe { res.assume_init() }
}

/// Generates a random instance of the given type, filling its bytes with RDRAND
///
/// ## Safety
///
/// Every random sequence must be a valid instance of the given type
pub unsafe fn randomize<'a, T: 'a + Sized>(iter: impl IntoIterator<Item = &'a mut T>) {
    for elem in iter {
        // Safety:
        // Guaranteed by the function's safety guards
        unsafe { *elem = rand() };
    }
}
