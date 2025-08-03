use crate::io::File;
use core::mem::MaybeUninit;

#[inline]
/// Fills the buffer with random bytes
///
/// ## Panics
///
/// Panics if the syscall fails.
/// This will happen if the input data is invalid or if randomness fails to be generated.
pub fn rand_fill(buf: &mut [u8]) {
    const RAND_FILE: &str = "/dev/rand";

    // FIXME: This is very inefficient and faillible if some other process
    // is using the rand file.
    let file = File::open(RAND_FILE).unwrap();

    let read_res = file.read(buf, 0);

    file.close().unwrap();

    let bytes_read = read_res.unwrap();
    assert!(bytes_read == buf.len(), "Failed to read random bytes");
}

#[must_use]
/// Generates a random value of the given type
///
/// ## Safety
///
/// Any random sequence of bytes should be a valid instance of the given type.
///
/// ## Panics
///
/// Panics if randomness fails to be generated.
pub unsafe fn rand<T: Sized>() -> T {
    let mut res = MaybeUninit::<T>::uninit();

    // Safety:
    // `MaybeUninit` guarantees that the layout is the same as `T`
    // so that the memory is valid for writes.
    let slice =
        unsafe { core::slice::from_raw_parts_mut(res.as_mut_ptr().cast::<u8>(), size_of::<T>()) };

    rand_fill(slice);

    // Safety:
    // We just initialized the value and because of the function's safety guards,
    // the result is a valid instance of the given type
    unsafe { res.assume_init() }
}
