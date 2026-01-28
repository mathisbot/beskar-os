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

#[must_use]
#[expect(clippy::missing_panics_doc, reason = "Never panics")]
/// Calculate the Internet checksum for a buffer.
///
/// The Internet checksum is the 16-bit one's complement of the one's complement
/// sum of the input buffer, treated as 16-bit big-endian values.
///
/// If the buffer has an odd length, the last byte is padded with zero.
pub fn checksum(buffer: &[u8]) -> u16 {
    let mut sum = 0_u32;

    // Sum all 16-bit words
    let mut i = 0;
    while i < buffer.len() {
        let word = if i + 1 < buffer.len() {
            u16::from_be_bytes([buffer[i], buffer[i + 1]])
        } else {
            // Odd byte: pad with zero
            u16::from_be_bytes([buffer[i], 0])
        };
        sum = sum.wrapping_add(u32::from(word));
        i += 2;
    }

    // Add carry bits
    while sum >> 16 > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    // Return one's complement
    !u16::try_from(sum).unwrap()
}

#[must_use]
#[expect(clippy::missing_panics_doc, reason = "Never panics")]
/// Calculate the Internet checksum with a pseudo-header.
///
/// Used for TCP and UDP checksums which include a pseudo-header containing
/// source address, destination address, protocol, and length.
pub fn checksum_with_pseudo(pseudo_header: &[u8], data: &[u8]) -> u16 {
    let mut sum = 0_u32;

    // Sum pseudo-header
    let mut i = 0;
    while i < pseudo_header.len() {
        let word = if i + 1 < pseudo_header.len() {
            u16::from_be_bytes([pseudo_header[i], pseudo_header[i + 1]])
        } else {
            u16::from_be_bytes([pseudo_header[i], 0])
        };
        sum = sum.wrapping_add(u32::from(word));
        i += 2;
    }

    // Sum data
    i = 0;
    while i < data.len() {
        let word = if i + 1 < data.len() {
            u16::from_be_bytes([data[i], data[i + 1]])
        } else {
            u16::from_be_bytes([data[i], 0])
        };
        sum = sum.wrapping_add(u32::from(word));
        i += 2;
    }

    // Add carry bits
    while sum >> 16 > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    // Return one's complement
    !u16::try_from(sum).unwrap()
}
