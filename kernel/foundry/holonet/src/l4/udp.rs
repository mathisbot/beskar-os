pub use core::net::{SocketAddr, SocketAddrV4, SocketAddrV6};

use crate::{
    NetworkError, NetworkResult,
    l3::ip::Ipv4Addr,
    utils::{checksum_with_pseudo, u16_from_inet_bytes, u16_to_inet_bytes},
};

/// Range of bytes for the source port field.
const SOURCE_PORT: core::ops::Range<usize> = 0..2;
/// Range of bytes for the destination port field.
const DEST_PORT: core::ops::Range<usize> = 2..4;
/// Range of bytes for the length field.
const LENGTH: core::ops::Range<usize> = 4..6;
/// Range of bytes for the checksum field.
const CHECKSUM: core::ops::Range<usize> = 6..8;
/// Length of the UDP header (fixed).
const HEADER_LEN: usize = 8;

/// A read/write wrapper around a UDP packet buffer.
#[derive(Debug, Clone)]
pub struct Packet<T: AsRef<[u8]>> {
    buffer: T,
}

impl<T: AsRef<[u8]>> Packet<T> {
    #[must_use]
    #[inline]
    /// Imbue a raw octet buffer with UDP packet structure.
    pub const fn new_unchecked(buffer: T) -> Self {
        Self { buffer }
    }

    #[inline]
    /// # Errors
    ///
    /// Returns `Invalid` if the buffer is too short.
    pub fn new(buffer: T) -> NetworkResult<Self> {
        let packet = Self::new_unchecked(buffer);
        packet.check_len()?;
        Ok(packet)
    }

    /// # Errors
    ///
    /// Returns `Invalid` if the buffer is too short.
    pub fn check_len(&self) -> NetworkResult<()> {
        (self.buffer.as_ref().len() >= HEADER_LEN)
            .then_some(())
            .ok_or(NetworkError::Invalid)
    }

    #[must_use]
    #[inline]
    /// Consumes the packet, returning the underlying buffer.
    pub fn into_inner(self) -> T {
        self.buffer
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the source port field.
    pub fn src_port(&self) -> u16 {
        let data = self.buffer.as_ref();
        u16_from_inet_bytes(data[SOURCE_PORT].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the destination port field.
    pub fn dst_port(&self) -> u16 {
        let data = self.buffer.as_ref();
        u16_from_inet_bytes(data[DEST_PORT].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the length field.
    pub fn len(&self) -> u16 {
        let data = self.buffer.as_ref();
        u16_from_inet_bytes(data[LENGTH].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the checksum field.
    pub fn checksum(&self) -> u16 {
        let data = self.buffer.as_ref();
        u16_from_inet_bytes(data[CHECKSUM].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    /// Return whether the packet has a valid checksum (non-zero for UDP/IPv4).
    pub fn has_checksum(&self) -> bool {
        self.checksum() != 0
    }

    #[must_use]
    #[inline]
    /// Return the payload.
    pub fn payload(&self) -> &[u8] {
        let data = self.buffer.as_ref();
        data[HEADER_LEN..]
            .split_at(usize::from(self.len()).saturating_sub(HEADER_LEN))
            .0
    }

    #[must_use]
    #[inline]
    /// Return whether this packet is empty (only contains the header).
    pub fn is_empty(&self) -> bool {
        usize::from(self.len()) <= HEADER_LEN
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    #[inline]
    /// Set the source port field.
    pub fn set_src_port(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[SOURCE_PORT].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the destination port field.
    pub fn set_dst_port(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[DEST_PORT].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the length field.
    pub fn set_len(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[LENGTH].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the checksum field.
    pub fn set_checksum(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[CHECKSUM].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Get a mutable reference to the payload.
    pub fn payload_mut(&mut self) -> &mut [u8] {
        let len = usize::from(self.len()).saturating_sub(HEADER_LEN);
        let data = self.buffer.as_mut();
        &mut data[HEADER_LEN..][..len]
    }

    /// Recalculate and set the UDP checksum with pseudo-header (IPv4).
    pub fn fill_checksum(&mut self, src_addr: Ipv4Addr, dst_addr: Ipv4Addr) {
        // Build pseudo-header
        let mut pseudo = [0u8; 12];
        pseudo[0..4].copy_from_slice(&src_addr.octets());
        pseudo[4..8].copy_from_slice(&dst_addr.octets());
        pseudo[8] = 0; // Reserved
        pseudo[9] = 17; // Protocol (UDP)
        pseudo[10..12].copy_from_slice(&u16_to_inet_bytes(self.len()));

        self.set_checksum(0);
        let data = self.buffer.as_ref();
        let cksum = checksum_with_pseudo(&pseudo, data);
        let _ = data;

        // UDP checksum should never be 0 in IPv4
        self.set_checksum(if cksum == 0 { 0xFFFF } else { cksum });
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for Packet<T> {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

/// A high-level representation of a UDP packet.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Repr {
    pub src_port: u16,
    pub dst_port: u16,
    pub payload_len: usize,
}

impl Repr {
    #[inline]
    /// Parse a UDP packet and return a high-level representation.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the packet is too short.
    pub fn parse<T: AsRef<[u8]> + ?Sized>(packet: &Packet<&T>) -> NetworkResult<Self> {
        packet.check_len()?;

        Ok(Self {
            src_port: packet.src_port(),
            dst_port: packet.dst_port(),
            payload_len: packet.payload().len(),
        })
    }

    #[must_use]
    #[inline]
    /// Return the length of a packet that will be emitted from this high-level representation.
    pub const fn buffer_len(&self) -> usize {
        HEADER_LEN + self.payload_len
    }

    /// Emit a high-level representation into a UDP packet.
    ///
    /// # Panics
    ///
    /// Panics if the packet buffer is too short.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) {
        assert!(packet.buffer.as_ref().len() >= self.buffer_len());
        packet.set_src_port(self.src_port);
        packet.set_dst_port(self.dst_port);
        packet.set_len(u16::try_from(HEADER_LEN + self.payload_len).unwrap());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec;

    static PACKET_BYTES: [u8; 8] = [
        0x00, 0x35, // Source port 53
        0x12, 0x34, // Destination port 4660
        0x00, 0x08, // Length 8
        0x00, 0x00, // Checksum 0
    ];

    #[test]
    fn test_src_dst_port() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.src_port(), 53);
        assert_eq!(packet.dst_port(), 4660);
    }

    #[test]
    fn test_len() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.len(), 8);
    }

    #[test]
    fn test_construct() {
        let mut bytes = vec![0u8; 8];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_src_port(53);
        packet.set_dst_port(4660);
        packet.set_len(8);
        packet.set_checksum(0);

        assert_eq!(packet.src_port(), 53);
        assert_eq!(packet.dst_port(), 4660);
        assert_eq!(packet.len(), 8);
    }
}
