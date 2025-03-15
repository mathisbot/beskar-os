use super::cpuid;
use core::mem::MaybeUninit;
use hyperdrive::once::Once;
use thiserror::Error;

static RDRAND_SUPPORT: Once<bool> = Once::uninit();

#[derive(Debug, Error)]
pub enum RandError {
    #[error("RDRAND is not supported")]
    RdrandNotSupported,
    #[error("RDRAND failed to generate random data")]
    RdrandFailed,
}

/// The maximum number of retries for RDRAND
///
/// The value is Intel's recommendation
const RETRY_LIMIT: u8 = 10;

fn rdrand(dst: &mut u64) -> Result<(), RandError> {
    for _ in 0..RETRY_LIMIT {
        if unsafe { core::arch::x86_64::_rdrand64_step(dst) } == 1 {
            return Ok(());
        }
    }
    Err(RandError::RdrandFailed)
}

#[must_use]
/// Generates a random instance of the given type, filling its bytes with RDRAND
///
/// ## Safety
///
/// Every random sequence of bits must be a valid instance of the given type
///
/// ## Panics
///
/// Panics if RDRAND is not supported or if it fails to generate random data.
/// See `rand_bytes`.
pub unsafe fn rand<T: Sized>() -> Result<T, RandError> {
    let mut res = MaybeUninit::<T>::uninit();

    // Safety:
    // `MaybeUninit` guarantees that the layout is the same as `T`
    // so that the memory is valid for writes.
    let slice =
        unsafe { core::slice::from_raw_parts_mut(res.as_mut_ptr().cast::<u8>(), size_of::<T>()) };

    rand_bytes(slice)?;

    // Safety:
    // We just initialized the value and because of the function's safety guards,
    // the result is a valid instance of the given type
    Ok(unsafe { res.assume_init() })
}

/// Generates random bytes using RDRAND
///
/// ## Panics
///
/// Panics if RDRAND is not supported or if it fails to generate random data
pub fn rand_bytes(bytes: &mut [u8]) -> Result<(), RandError> {
    if !rdrand_supported() {
        return Err(RandError::RdrandNotSupported);
    }

    // It is safe to cast 8 packed `u8`s to a `u64`
    let (start_u8, middle_u64, end_u8) = unsafe { bytes.align_to_mut::<u64>() };

    // Quickly fill the middle part of the slice with random u64s
    for qword in middle_u64 {
        rdrand(qword)?;
    }

    // Now fill the unaligned start and end of the slice
    let mut randomized = 0;

    if !start_u8.is_empty() {
        // One call to `rdrand` is enough as `start_u8.len()` is less than 8
        rdrand(&mut randomized)?;
        start_u8.copy_from_slice(&randomized.to_ne_bytes()[..start_u8.len()]);
    }

    if !end_u8.is_empty() {
        // One call to `rdrand` is enough as `end_u8.len()` is less than 8
        rdrand(&mut randomized)?;
        end_u8.copy_from_slice(&randomized.to_ne_bytes()[..end_u8.len()]);
    }

    Ok(())
}

#[inline]
pub fn rdrand_supported() -> bool {
    RDRAND_SUPPORT.call_once(|| cpuid::check_feature(cpuid::CpuFeature::RDRAND));
    *RDRAND_SUPPORT.get().unwrap()
}
