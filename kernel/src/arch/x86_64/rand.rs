use super::cpuid;
use hyperdrive::once::Once;
use thiserror::Error;

static RDRAND_SUPPORT: Once<bool> = Once::uninit();
static RDSEED_SUPPORT: Once<bool> = Once::uninit();

#[derive(Debug, Error)]
pub enum RandError {
    #[error("RDRAND is not supported")]
    RdrandNotSupported,
    #[error("RDRAND failed to generate random data")]
    RdrandFailed,
    #[error("RDSEED is not supported")]
    RdseedNotSupported,
    #[error("RDSEED failed to generate random data")]
    RdseedFailed,
}

/// The maximum number of retries for RDRAND
///
/// The value is Intel's recommendation
const RETRY_LIMIT: u8 = 10;

/// The maximum number of retries for RDSEED
///
/// RDSEED can take longer than RDRAND as it sources true entropy
const RDSEED_RETRY_LIMIT: u16 = 100;

fn rdrand(dst: &mut u64) -> Result<(), RandError> {
    for _ in 0..RETRY_LIMIT {
        if unsafe { core::arch::x86_64::_rdrand64_step(dst) } == 1 {
            return Ok(());
        }
    }
    Err(RandError::RdrandFailed)
}

fn rdseed(dst: &mut u64) -> Result<(), RandError> {
    for _ in 0..RDSEED_RETRY_LIMIT {
        if unsafe { core::arch::x86_64::_rdseed64_step(dst) } == 1 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(RandError::RdseedFailed)
}

/// Generates random bytes using RDRAND
///
/// # Errors
///
/// Returns `RandError::RdrandNotSupported` if RDRAND is not supported
/// and `RandError::RdrandFailed` if RDRAND fails to generate random data.
pub fn rand_bytes(bytes: &mut [u8]) -> Result<(), RandError> {
    if !rdrand_supported() {
        return Err(RandError::RdrandNotSupported);
    }

    rand_bytes_with(bytes, rdrand)
}

/// Generates cryptographic seed bytes using RDSEED
///
/// RDSEED provides access to a hardware entropy source, making it suitable
/// for seeding PRNGs or generating cryptographic keys. It's slower than RDRAND
/// but provides higher quality randomness.
///
/// # Errors
///
/// Returns `RandError::RdseedNotSupported` if RDSEED is not supported
/// and `RandError::RdseedFailed` if RDSEED fails to generate random data.
pub fn rand_seed_bytes(bytes: &mut [u8]) -> Result<(), RandError> {
    if !rdseed_supported() {
        return Err(RandError::RdseedNotSupported);
    }

    rand_bytes_with(bytes, rdseed)
}

/// Generic implementation for filling bytes using a random number generator
fn rand_bytes_with(
    bytes: &mut [u8],
    mut rng: impl FnMut(&mut u64) -> Result<(), RandError>,
) -> Result<(), RandError> {
    // It is safe to cast 8 packed `u8`s to a `u64`
    let (start_u8, middle_u64, end_u8) = unsafe { bytes.align_to_mut::<u64>() };

    // Quickly fill the middle part of the slice with random u64s
    for qword in middle_u64 {
        rng(qword)?;
    }

    // Now fill the unaligned start and end of the slice
    let mut randomized = 0;

    if !start_u8.is_empty() {
        // One call to `rng` is enough as `start_u8.len()` is less than 8
        rng(&mut randomized)?;
        start_u8.copy_from_slice(&randomized.to_ne_bytes()[..start_u8.len()]);
    }

    if !end_u8.is_empty() {
        // One call to `rng` is enough as `end_u8.len()` is less than 8
        rng(&mut randomized)?;
        end_u8.copy_from_slice(&randomized.to_ne_bytes()[..end_u8.len()]);
    }

    Ok(())
}

#[inline]
pub fn rdrand_supported() -> bool {
    RDRAND_SUPPORT.call_once(|| cpuid::check_feature(cpuid::CpuFeature::RDRAND));
    *RDRAND_SUPPORT.get().unwrap()
}

#[inline]
pub fn rdseed_supported() -> bool {
    RDSEED_SUPPORT.call_once(|| cpuid::check_feature(cpuid::CpuFeature::RDSEED));
    *RDSEED_SUPPORT.get().unwrap()
}
