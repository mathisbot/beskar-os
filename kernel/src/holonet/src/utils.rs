#![allow(dead_code)]

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
