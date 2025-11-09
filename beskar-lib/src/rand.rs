use crate::error::{IoError, IoErrorKind, IoResult};
use crate::io::{File, Read};
use core::mem::{self, MaybeUninit};

/// Fills the buffer with random bytes.
///
/// # Errors
///
/// Returns an error if the random device cannot be read.
pub fn rand_fill(buf: &mut [u8]) -> IoResult<()> {
    const RAND_FILE: &str = "/dev/rand";

    // Open the random device
    let mut file = File::open(RAND_FILE).map_err(|_| IoError::new(IoErrorKind::Other))?;

    // Use read_exact semantics to ensure buffer is fully filled
    file.read_exact(buf)
        .map_err(|_| IoError::new(IoErrorKind::UnexpectedEof))?;

    // Close the device (best-effort)
    let _ = file.close();

    Ok(())
}

/// Generates a random value of the given type.
///
/// # Errors
///
/// Returns an error if the random device cannot be read.
///
/// # Safety
///
/// The returned value of type `T` is produced from raw random bytes.
/// Any random sequence of bytes must be valid for type `T`.
pub unsafe fn rand<T: Sized>() -> IoResult<T> {
    let mut uninit = MaybeUninit::<T>::uninit();
    let buf = unsafe {
        core::slice::from_raw_parts_mut(uninit.as_mut_ptr().cast::<u8>(), mem::size_of::<T>())
    };

    rand_fill(buf)?;

    // Safety: buffer just initialized with bytes from the random device.
    // As per the function safety contract, any byte sequence is valid for `T`.
    let val = unsafe { uninit.assume_init() };
    Ok(val)
}
