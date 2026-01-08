use crate::{
    NetworkError, NetworkResult,
    utils::{
        checksum, u16_from_inet_bytes, u16_to_inet_bytes, u32_from_inet_bytes, u32_to_inet_bytes,
    },
};
pub use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Range of bytes for the total length field.
const TOTAL_LEN: core::ops::Range<usize> = 2..4;
/// Range of bytes for the identification field.
const IDENTIFICATION: core::ops::Range<usize> = 4..6;
/// Range of bytes for the flags and fragment offset field.
const FLAGS_FRAG_OFFSET: core::ops::Range<usize> = 6..8;
/// Range of bytes for the TTL field.
const TTL: core::ops::Range<usize> = 8..9;
/// Range of bytes for the protocol field.
const PROTOCOL: core::ops::Range<usize> = 9..10;
/// Range of bytes for the header checksum field.
const HEADER_CHECKSUM: core::ops::Range<usize> = 10..12;
/// Range of bytes for the source address field.
const SOURCE_ADDR: core::ops::Range<usize> = 12..16;
/// Range of bytes for the destination address field.
const DEST_ADDR: core::ops::Range<usize> = 16..20;
/// Length of the IPv4 header (minimum)
const HEADER_LEN: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
/// IPv4 protocol number.
pub enum Protocol {
    Icmp = 1,
    Igmp = 2,
    Tcp = 6,
    Udp = 17,
}

impl TryFrom<u8> for Protocol {
    type Error = NetworkError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Icmp),
            2 => Ok(Self::Igmp),
            6 => Ok(Self::Tcp),
            17 => Ok(Self::Udp),
            _ => Err(NetworkError::Invalid),
        }
    }
}

impl From<Protocol> for u8 {
    fn from(value: Protocol) -> Self {
        value as Self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// IPv4 header flags.
pub struct Flags {
    /// Reserved bit (must be 0)
    pub reserved: bool,
    /// Don't fragment flag
    pub dont_fragment: bool,
    /// More fragments flag
    pub more_fragments: bool,
}

impl Flags {
    /// Parse flags from the flags/fragment offset byte.
    #[must_use]
    pub const fn from_bits(value: u8) -> Self {
        Self {
            reserved: (value & 0x80) != 0,
            dont_fragment: (value & 0x40) != 0,
            more_fragments: (value & 0x20) != 0,
        }
    }

    /// Convert flags to byte representation.
    #[must_use]
    pub const fn to_bits(self) -> u8 {
        let mut bits = 0;
        if self.reserved {
            bits |= 0x80;
        }
        if self.dont_fragment {
            bits |= 0x40;
        }
        if self.more_fragments {
            bits |= 0x20;
        }
        bits
    }
}

/// A read/write wrapper around an IPv4 packet buffer.
#[derive(Debug, Clone)]
pub struct Packet<T: AsRef<[u8]>> {
    buffer: T,
}

impl<T: AsRef<[u8]>> Packet<T> {
    #[must_use]
    #[inline]
    /// Imbue a raw octet buffer with IPv4 packet structure.
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
    /// Return the length of the IPv4 header.
    pub fn header_len(&self) -> usize {
        let data = self.buffer.as_ref();
        ((data[0] & 0x0F) as usize) * 4
    }

    #[must_use]
    #[inline]
    /// Return the version field.
    pub fn version(&self) -> u8 {
        self.buffer.as_ref()[0] >> 4
    }

    #[must_use]
    #[inline]
    /// Return the DSCP field.
    pub fn dscp(&self) -> u8 {
        self.buffer.as_ref()[1] >> 2
    }

    #[must_use]
    #[inline]
    /// Return the ECN field.
    pub fn ecn(&self) -> u8 {
        self.buffer.as_ref()[1] & 0x03
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the total length field.
    pub fn total_len(&self) -> u16 {
        let data = self.buffer.as_ref();
        u16_from_inet_bytes(data[TOTAL_LEN].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the identification field.
    pub fn identification(&self) -> u16 {
        let data = self.buffer.as_ref();
        u16_from_inet_bytes(data[IDENTIFICATION].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    /// Return the flags field.
    pub fn flags(&self) -> Flags {
        let data = self.buffer.as_ref();
        Flags::from_bits(data[FLAGS_FRAG_OFFSET.start])
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the fragment offset field (in units of 8 bytes).
    pub fn fragment_offset(&self) -> u16 {
        let data = self.buffer.as_ref();
        let raw = u16_from_inet_bytes(data[FLAGS_FRAG_OFFSET].try_into().unwrap());
        raw & 0x1FFF
    }

    #[must_use]
    #[inline]
    /// Return the TTL field.
    pub fn ttl(&self) -> u8 {
        self.buffer.as_ref()[TTL.start]
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the protocol field.
    pub fn protocol(&self) -> Protocol {
        Protocol::try_from(self.buffer.as_ref()[PROTOCOL.start]).unwrap()
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the header checksum field.
    pub fn checksum(&self) -> u16 {
        let data = self.buffer.as_ref();
        u16_from_inet_bytes(data[HEADER_CHECKSUM].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the source address field.
    pub fn src_addr(&self) -> Ipv4Addr {
        let data = self.buffer.as_ref();
        Ipv4Addr::from(u32_from_inet_bytes(data[SOURCE_ADDR].try_into().unwrap()))
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the destination address field.
    pub fn dst_addr(&self) -> Ipv4Addr {
        let data = self.buffer.as_ref();
        Ipv4Addr::from(u32_from_inet_bytes(data[DEST_ADDR].try_into().unwrap()))
    }

    #[must_use]
    #[inline]
    /// Return the payload.
    pub fn payload(&self) -> &[u8] {
        let data = self.buffer.as_ref();
        &data[self.header_len()..]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    #[inline]
    /// Set the version and header length fields.
    pub fn set_version_and_header_len(&mut self, version: u8, header_len: u8) {
        let data = self.buffer.as_mut();
        data[0] = (version << 4) | (header_len & 0x0F);
    }

    #[inline]
    /// Set the DSCP and ECN fields.
    pub fn set_dscp_ecn(&mut self, dscp: u8, ecn: u8) {
        let data = self.buffer.as_mut();
        data[1] = (dscp << 2) | (ecn & 0x03);
    }

    #[inline]
    /// Set the total length field.
    pub fn set_total_len(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[TOTAL_LEN].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the identification field.
    pub fn set_identification(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[IDENTIFICATION].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the flags and fragment offset fields.
    pub fn set_flags_and_fragment_offset(&mut self, flags: Flags, fragment_offset: u16) {
        let value = (u16::from(flags.to_bits()) << 5) | (fragment_offset & 0x1FFF);
        let data = self.buffer.as_mut();
        data[FLAGS_FRAG_OFFSET].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the TTL field.
    pub fn set_ttl(&mut self, value: u8) {
        self.buffer.as_mut()[TTL.start] = value;
    }

    #[inline]
    /// Set the protocol field.
    pub fn set_protocol(&mut self, value: Protocol) {
        self.buffer.as_mut()[PROTOCOL.start] = value.into();
    }

    #[inline]
    /// Set the header checksum field.
    pub fn set_checksum(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[HEADER_CHECKSUM].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the source address field.
    pub fn set_src_addr(&mut self, value: Ipv4Addr) {
        let data = self.buffer.as_mut();
        data[SOURCE_ADDR].copy_from_slice(&u32_to_inet_bytes(u32::from(value)));
    }

    #[inline]
    /// Set the destination address field.
    pub fn set_dst_addr(&mut self, value: Ipv4Addr) {
        let data = self.buffer.as_mut();
        data[DEST_ADDR].copy_from_slice(&u32_to_inet_bytes(u32::from(value)));
    }

    #[inline]
    /// Get a mutable reference to the payload.
    pub fn payload_mut(&mut self) -> &mut [u8] {
        let header_len = self.header_len();
        let data = self.buffer.as_mut();
        &mut data[header_len..]
    }

    /// Recalculate and set the header checksum.
    pub fn fill_checksum(&mut self) {
        self.set_checksum(0);
        let header_len = self.header_len();
        let data = self.buffer.as_ref();
        let cksum = checksum(&data[..header_len]);
        let _ = data;
        self.set_checksum(cksum);
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for Packet<T> {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

/// A high-level representation of an IPv4 packet header.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Repr {
    pub src_addr: Ipv4Addr,
    pub dst_addr: Ipv4Addr,
    pub protocol: Protocol,
    pub payload_len: usize,
    pub ttl: u8,
    pub flags: Flags,
}

impl Repr {
    #[inline]
    /// Parse an IPv4 packet and return a high-level representation.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the packet is too short or has invalid version.
    /// Returns `Unsupported` if the protocol is not recognized.
    pub fn parse<T: AsRef<[u8]> + ?Sized>(packet: &Packet<&T>) -> NetworkResult<Self> {
        packet.check_len()?;

        if packet.version() != 4 {
            return Err(NetworkError::Invalid);
        }

        Ok(Self {
            src_addr: packet.src_addr(),
            dst_addr: packet.dst_addr(),
            protocol: packet.protocol(),
            payload_len: packet.payload().len(),
            ttl: packet.ttl(),
            flags: packet.flags(),
        })
    }

    #[must_use]
    #[inline]
    /// Return the length of a header that will be emitted from this high-level representation.
    pub const fn buffer_len(&self) -> usize {
        HEADER_LEN + self.payload_len
    }

    /// Emit a high-level representation into an IPv4 packet.
    ///
    /// # Panics
    ///
    /// Panics if the packet buffer is too short.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) {
        assert!(packet.buffer.as_ref().len() >= self.buffer_len());
        packet.set_version_and_header_len(4, 5); // Version 4, IHL 5 (20 bytes)
        packet.set_dscp_ecn(0, 0);
        packet.set_total_len(u16::try_from(HEADER_LEN + self.payload_len).unwrap());
        packet.set_identification(0);
        packet.set_flags_and_fragment_offset(self.flags, 0);
        packet.set_ttl(self.ttl);
        packet.set_protocol(self.protocol);
        packet.set_src_addr(self.src_addr);
        packet.set_dst_addr(self.dst_addr);
        packet.fill_checksum();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec;

    static PACKET_BYTES: [u8; 20] = [
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x40, 0x00, 0x40, 0x06, 0x3c, 0x1c, 0xc0, 0xa8, 0x00,
        0x01, 0xc0, 0xa8, 0x00, 0x02,
    ];

    #[test]
    fn test_version() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.version(), 4);
    }

    #[test]
    fn test_header_len() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.header_len(), 20);
    }

    #[test]
    fn test_src_dst_addr() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.src_addr(), Ipv4Addr::new(192, 168, 0, 1));
        assert_eq!(packet.dst_addr(), Ipv4Addr::new(192, 168, 0, 2));
    }

    #[test]
    fn test_protocol() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.protocol(), Protocol::Tcp);
    }

    #[test]
    fn test_flags() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        let flags = packet.flags();
        assert!(!flags.reserved);
        assert!(flags.dont_fragment);
        assert!(!flags.more_fragments);
    }

    #[test]
    fn test_construct() {
        let mut bytes = vec![0u8; 20];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_version_and_header_len(4, 5);
        packet.set_dscp_ecn(0, 0);
        packet.set_total_len(20);
        packet.set_identification(0x1234);
        packet.set_flags_and_fragment_offset(
            Flags {
                reserved: false,
                dont_fragment: true,
                more_fragments: false,
            },
            0,
        );
        packet.set_ttl(64);
        packet.set_protocol(Protocol::Tcp);
        packet.set_src_addr(Ipv4Addr::new(192, 168, 0, 1));
        packet.set_dst_addr(Ipv4Addr::new(192, 168, 0, 2));
        packet.fill_checksum();

        // Check that values are set correctly
        assert_eq!(packet.version(), 4);
        assert_eq!(packet.total_len(), 20);
        assert_eq!(packet.identification(), 0x1234);
    }
}
