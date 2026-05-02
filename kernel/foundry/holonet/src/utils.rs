use crate::{NetworkError, NetworkResult};
use core::slice::SliceIndex;

#[must_use]
#[inline]
/// Convert bytes into a u16 value in network byte order (big-endian).
pub const fn u16_from_inet_bytes(bytes: [u8; 2]) -> u16 {
    u16::from_be_bytes(bytes)
}
#[must_use]
#[inline]
/// Convert a u16 value into bytes in network byte order (big-endian).
pub const fn u16_to_inet_bytes(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}

#[must_use]
#[inline]
/// Convert bytes into a u32 value in network byte order (big-endian).
pub const fn u32_from_inet_bytes(bytes: [u8; 4]) -> u32 {
    u32::from_be_bytes(bytes)
}
#[must_use]
#[inline]
/// Convert a u32 value into bytes in network byte order (big-endian).
pub const fn u32_to_inet_bytes(value: u32) -> [u8; 4] {
    value.to_be_bytes()
}

#[must_use]
#[inline]
/// Convert bytes into a u64 value in network byte order (big-endian).
pub const fn u64_from_inet_bytes(bytes: [u8; 8]) -> u64 {
    u64::from_be_bytes(bytes)
}
#[must_use]
#[inline]
/// Convert a u64 value into bytes in network byte order (big-endian).
pub const fn u64_to_inet_bytes(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}

#[inline]
fn checksum_accumulate(mut sum: u32, buffer: &[u8]) -> u32 {
    let mut chunks = buffer.chunks_exact(2);
    for chunk in &mut chunks {
        let word = u16::from_be_bytes([chunk[0], chunk[1]]);
        sum = sum.wrapping_add(u32::from(word));
    }

    if let [last] = chunks.remainder() {
        let word = u16::from_be_bytes([*last, 0]);
        sum = sum.wrapping_add(u32::from(word));
    }

    sum
}

#[must_use]
#[inline]
const fn checksum_finalize(mut sum: u32) -> u16 {
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    // The `& 0xFFFF` is redundant but needed for optimization apparently
    let folded = (sum & 0xFFFF) as u16;
    !folded
}

#[must_use]
/// Calculate the Internet checksum for a buffer.
///
/// The Internet checksum is the 16-bit one's complement of the one's complement
/// sum of the input buffer, treated as 16-bit big-endian values.
///
/// If the buffer has an odd length, the last byte is padded with zero.
pub fn checksum(buffer: &[u8]) -> u16 {
    let acc = checksum_accumulate(0, buffer);
    checksum_finalize(acc)
}

#[must_use]
/// Calculate the Internet checksum with a pseudo-header.
pub fn checksum_with_pseudo(pseudo_header: &[u8], data: &[u8]) -> u16 {
    let pseudo = checksum_accumulate(0, pseudo_header);
    let acc = checksum_accumulate(pseudo, data);
    checksum_finalize(acc)
}

#[inline]
pub(crate) const fn ensure_len(buffer: &[u8], len: usize) -> NetworkResult<()> {
    if buffer.len() >= len {
        Ok(())
    } else {
        Err(NetworkError::Truncated)
    }
}

#[inline]
pub(crate) fn slice<I: SliceIndex<[u8]>>(
    buffer: &[u8],
    index: I,
) -> NetworkResult<&<I as SliceIndex<[u8]>>::Output> {
    buffer.get(index).ok_or(NetworkError::Truncated)
}

#[inline]
pub(crate) fn read_u8(buffer: &[u8], position: usize) -> NetworkResult<u8> {
    let slot = slice(buffer, position)?;
    Ok(*slot)
}

#[inline]
pub(crate) fn read_u16(buffer: &[u8], position: usize) -> NetworkResult<u16> {
    read_array::<2>(buffer, position).map(u16_from_inet_bytes)
}

#[inline]
pub(crate) fn read_u32(buffer: &[u8], position: usize) -> NetworkResult<u32> {
    read_array::<4>(buffer, position).map(u32_from_inet_bytes)
}

#[inline]
pub(crate) fn read_array<const N: usize>(buffer: &[u8], position: usize) -> NetworkResult<[u8; N]> {
    let index = position..position + N;
    let bytes = slice(buffer, index)?;
    bytes.try_into().map_err(|_| NetworkError::Invalid) // Cannot fail
}

#[inline]
pub(crate) fn slice_mut<I: SliceIndex<[u8]>>(
    buffer: &mut [u8],
    index: I,
) -> NetworkResult<&mut <I as SliceIndex<[u8]>>::Output> {
    buffer.get_mut(index).ok_or(NetworkError::Truncated)
}

#[inline]
pub(crate) fn write_u8(buffer: &mut [u8], position: usize, value: u8) -> NetworkResult<()> {
    let slot = slice_mut(buffer, position)?;
    *slot = value;
    Ok(())
}

#[inline]
pub(crate) fn write_u16(buffer: &mut [u8], position: usize, value: u16) -> NetworkResult<()> {
    write_slice(buffer, position, &u16_to_inet_bytes(value))
}

#[inline]
pub(crate) fn write_u32(buffer: &mut [u8], position: usize, value: u32) -> NetworkResult<()> {
    write_slice(buffer, position, &u32_to_inet_bytes(value))
}

#[inline]
pub(crate) fn write_slice(buffer: &mut [u8], position: usize, src: &[u8]) -> NetworkResult<()> {
    let index = position..position + src.len();
    let dst = slice_mut(buffer, index)?;
    dst.copy_from_slice(src);
    Ok(())
}

#[inline]
pub(crate) fn usize_to_u16(value: usize) -> NetworkResult<u16> {
    u16::try_from(value).map_err(|_| NetworkError::Oversized)
}
