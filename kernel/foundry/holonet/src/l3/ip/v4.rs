use super::Protocol;
use crate::{
    NetworkError, NetworkResult,
    utils::{
        checksum, ensure_len, read_u8, read_u16, read_u32, slice, slice_mut, usize_to_u16,
        write_u8, write_u16, write_u32,
    },
};
pub use core::net::Ipv4Addr;

/// Range of bytes for the total length field.
const TOTAL_LEN: usize = 2;
/// Range of bytes for the identification field.
const IDENTIFICATION: usize = 4;
/// Range of bytes for the flags and fragment offset field.
const FLAGS_FRAG_OFFSET: usize = 6;
/// Range of bytes for the TTL field.
const TTL: usize = 8;
/// Range of bytes for the protocol field.
const PROTOCOL: usize = 9;
/// Range of bytes for the header checksum field.
const HEADER_CHECKSUM: usize = 10;
/// Range of bytes for the source address field.
const SOURCE_ADDR: usize = 12;
/// Range of bytes for the destination address field.
const DEST_ADDR: usize = 16;

/// Length of the IPv4 header (minimum)
pub const HEADER_LEN: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// IPv4 header flags.
pub struct Flags(u8);

impl Flags {
    // const RESERVED_MBZ_BIT: u8 = 0x80;
    /// Router must not fragment the packet.
    const DONT_FRAGMENT_BIT: u8 = 0x40;
    /// More fragments follow this one.
    const MORE_FRAGMENTS_BIT: u8 = 0x20;

    const ALL: u8 = Self::DONT_FRAGMENT_BIT | Self::MORE_FRAGMENTS_BIT;

    #[must_use]
    #[inline]
    pub const fn new(dont_fragment: bool, more_fragments: bool) -> Self {
        let mut raw = 0;

        if dont_fragment {
            raw |= Self::DONT_FRAGMENT_BIT;
        }
        if more_fragments {
            raw |= Self::MORE_FRAGMENTS_BIT;
        }

        Self(raw)
    }

    /// Parse flags from the flags/fragment offset byte.
    #[must_use]
    #[inline]
    pub const fn from_bits(value: u8) -> Self {
        Self(value & Self::ALL)
    }

    /// Convert flags to byte representation.
    #[must_use]
    #[inline]
    pub const fn to_bits(self) -> u8 {
        self.0
    }

    /// Return true if the "Don't Fragment" flag is set.
    #[must_use]
    #[inline]
    pub const fn dont_fragment(self) -> bool {
        self.0 & Self::DONT_FRAGMENT_BIT != 0
    }

    /// Return true if the "More Fragments" flag is set.
    #[must_use]
    #[inline]
    pub const fn more_fragments(self) -> bool {
        self.0 & Self::MORE_FRAGMENTS_BIT != 0
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
    /// Returns `Invalid` if the buffer is too short or if IHL/total-length fields are inconsistent.
    pub fn check_len(&self) -> NetworkResult<()> {
        let data = self.buffer.as_ref();

        let total_len = usize::from(read_u16(data, TOTAL_LEN)?);
        if total_len < self.header_len()? {
            return Err(NetworkError::Invalid);
        }

        ensure_len(data, total_len)?;

        Ok(())
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
    pub fn header_len(&self) -> NetworkResult<usize> {
        let ihl = read_u8(self.buffer.as_ref(), 0)? & 0x0F;
        if ihl < 5 {
            return Err(NetworkError::Invalid);
        }
        Ok(usize::from(ihl) * 4)
    }

    #[must_use]
    #[inline]
    /// Return the version field.
    pub fn version(&self) -> NetworkResult<u8> {
        let v = read_u8(self.buffer.as_ref(), 0)? >> 4;
        if v != 4 {
            return Err(NetworkError::Invalid);
        }
        Ok(v)
    }

    #[must_use]
    #[inline]
    /// Return the DSCP field.
    pub fn dscp(&self) -> NetworkResult<u8> {
        Ok(read_u8(self.buffer.as_ref(), 1)? >> 2)
    }

    #[must_use]
    #[inline]
    /// Return the ECN field.
    pub fn ecn(&self) -> NetworkResult<u8> {
        Ok(read_u8(self.buffer.as_ref(), 1)? & 0x03)
    }

    #[must_use]
    #[inline]
    /// Return the total length field.
    pub fn total_len(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), TOTAL_LEN)
    }

    #[must_use]
    #[inline]
    /// Return the identification field.
    pub fn identification(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), IDENTIFICATION)
    }

    #[must_use]
    #[inline]
    /// Return the flags field.
    pub fn flags(&self) -> NetworkResult<Flags> {
        Ok(Flags::from_bits(read_u8(
            self.buffer.as_ref(),
            FLAGS_FRAG_OFFSET,
        )?))
    }

    #[must_use]
    #[inline]
    /// Return the fragment offset field (in units of 8 bytes).
    pub fn fragment_offset(&self) -> NetworkResult<u16> {
        Ok(read_u16(self.buffer.as_ref(), FLAGS_FRAG_OFFSET)? & 0x1FFF)
    }

    #[must_use]
    #[inline]
    /// Return the TTL field.
    pub fn ttl(&self) -> NetworkResult<u8> {
        read_u8(self.buffer.as_ref(), TTL)
    }

    #[must_use]
    #[inline]
    /// Return the raw protocol field value.
    pub fn protocol_raw(&self) -> NetworkResult<u8> {
        read_u8(self.buffer.as_ref(), PROTOCOL)
    }

    #[must_use]
    #[inline]
    /// Return the protocol field.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the value does not map to a known protocol.
    pub fn protocol(&self) -> NetworkResult<Protocol> {
        Protocol::try_from(self.protocol_raw()?)
    }

    #[must_use]
    #[inline]
    /// Return the header checksum field.
    pub fn checksum(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), HEADER_CHECKSUM)
    }

    #[must_use]
    #[inline]
    /// Return the source address field.
    pub fn src_addr(&self) -> NetworkResult<Ipv4Addr> {
        read_u32(self.buffer.as_ref(), SOURCE_ADDR).map(Ipv4Addr::from)
    }

    #[must_use]
    #[inline]
    /// Return the destination address field.
    pub fn dst_addr(&self) -> NetworkResult<Ipv4Addr> {
        read_u32(self.buffer.as_ref(), DEST_ADDR).map(Ipv4Addr::from)
    }

    #[must_use]
    #[inline]
    /// Return the payload.
    pub fn payload(&self) -> NetworkResult<&[u8]> {
        let data = self.buffer.as_ref();
        let header_len = self.header_len()?;
        let total_len = usize::from(self.total_len()?);
        slice(data, header_len..total_len)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    #[inline]
    /// Set the version and header length fields.
    pub fn set_version_and_header_len(&mut self, version: u8, header_len: u8) -> NetworkResult<()> {
        let data = self.buffer.as_mut();
        write_u8(data, 0, ((version & 0x0F) << 4) | (header_len & 0x0F))
    }

    #[inline]
    /// Set the DSCP and ECN fields.
    pub fn set_dscp_ecn(&mut self, dscp: u8, ecn: u8) -> NetworkResult<()> {
        let data = self.buffer.as_mut();
        write_u8(data, 1, ((dscp & 0x3F) << 2) | (ecn & 0x03))
    }

    #[inline]
    /// Set the total length field.
    pub fn set_total_len(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), TOTAL_LEN, value)
    }

    #[inline]
    /// Set the identification field.
    pub fn set_identification(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), IDENTIFICATION, value)
    }

    #[inline]
    /// Set the flags and fragment offset fields.
    pub fn set_flags_and_fragment_offset(
        &mut self,
        flags: Flags,
        fragment_offset: u16,
    ) -> NetworkResult<()> {
        let value = (u16::from(flags.to_bits()) << 8) | (fragment_offset & 0x1FFF);
        write_u16(self.buffer.as_mut(), FLAGS_FRAG_OFFSET, value)
    }

    #[inline]
    /// Set the TTL field.
    pub fn set_ttl(&mut self, value: u8) -> NetworkResult<()> {
        write_u8(self.buffer.as_mut(), TTL, value)
    }

    #[inline]
    /// Set the protocol field.
    pub fn set_protocol(&mut self, value: Protocol) -> NetworkResult<()> {
        write_u8(self.buffer.as_mut(), PROTOCOL, value.into())
    }

    #[inline]
    /// Set the header checksum field.
    pub fn set_checksum(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), HEADER_CHECKSUM, value)
    }

    #[inline]
    /// Set the source address field.
    pub fn set_src_addr(&mut self, value: Ipv4Addr) -> NetworkResult<()> {
        write_u32(self.buffer.as_mut(), SOURCE_ADDR, u32::from(value))
    }

    #[inline]
    /// Set the destination address field.
    pub fn set_dst_addr(&mut self, value: Ipv4Addr) -> NetworkResult<()> {
        write_u32(self.buffer.as_mut(), DEST_ADDR, u32::from(value))
    }

    #[inline]
    /// Get a mutable reference to the payload.
    pub fn payload_mut(&mut self) -> NetworkResult<&mut [u8]> {
        let data = self.buffer.as_ref();
        let header_len = self.header_len()?;
        let total_len = usize::from(self.total_len()?);
        ensure_len(data, total_len)?;
        slice_mut(self.buffer.as_mut(), header_len..total_len)
    }

    /// Recalculate and set the header checksum.
    pub fn fill_checksum(&mut self) -> NetworkResult<()> {
        self.set_checksum(0)?;
        let header_len = self.header_len()?;
        let data = self.buffer.as_ref();
        ensure_len(data, header_len)?;
        let cksum = checksum(&data[..header_len]);
        self.set_checksum(cksum)
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

        if packet.version()? != 4 {
            return Err(NetworkError::Invalid);
        }

        let protocol = packet.protocol().map_err(|_| NetworkError::Unsupported)?;

        Ok(Self {
            src_addr: packet.src_addr()?,
            dst_addr: packet.dst_addr()?,
            protocol,
            payload_len: packet.payload()?.len(),
            ttl: packet.ttl()?,
            flags: packet.flags()?,
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
    /// # Errors
    ///
    /// Returns `Truncated` if the packet buffer is too short.
    /// Returns `Oversized` if the payload length does not fit into the IPv4 length field.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) -> NetworkResult<()> {
        ensure_len(packet.buffer.as_ref(), self.buffer_len())?;
        packet.set_version_and_header_len(4, 5)?; // Version 4, IHL 5 (20 bytes)
        packet.set_dscp_ecn(0, 0)?;
        packet.set_total_len(usize_to_u16(HEADER_LEN + self.payload_len)?)?;
        packet.set_identification(0)?;
        packet.set_flags_and_fragment_offset(self.flags, 0)?;
        packet.set_ttl(self.ttl)?;
        packet.set_protocol(self.protocol)?;
        packet.set_src_addr(self.src_addr)?;
        packet.set_dst_addr(self.dst_addr)?;
        packet.fill_checksum()
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
        assert_eq!(packet.version(), Ok(4));
    }

    #[test]
    fn test_header_len() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.header_len(), Ok(20));
    }

    #[test]
    fn test_src_dst_addr() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.src_addr(), Ok(Ipv4Addr::new(192, 168, 0, 1)));
        assert_eq!(packet.dst_addr(), Ok(Ipv4Addr::new(192, 168, 0, 2)));
    }

    #[test]
    fn test_protocol() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.protocol(), Ok(Protocol::Tcp));
    }

    #[test]
    fn test_flags() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        let flags = packet.flags().unwrap();
        assert!(flags.dont_fragment());
        assert!(!flags.more_fragments());
    }

    #[test]
    fn test_construct() {
        let mut bytes = vec![0u8; 20];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_version_and_header_len(4, 5).unwrap();
        packet.set_dscp_ecn(0, 0).unwrap();
        packet.set_total_len(20).unwrap();
        packet.set_identification(0x1234).unwrap();
        packet
            .set_flags_and_fragment_offset(Flags::new(true, false), 0)
            .unwrap();
        packet.set_ttl(64).unwrap();
        packet.set_protocol(Protocol::Tcp).unwrap();
        packet.set_src_addr(Ipv4Addr::new(192, 168, 0, 1)).unwrap();
        packet.set_dst_addr(Ipv4Addr::new(192, 168, 0, 2)).unwrap();
        packet.fill_checksum().unwrap();

        // Check that values are set correctly
        assert_eq!(packet.version(), Ok(4));
        assert_eq!(packet.total_len(), Ok(20));
        assert_eq!(packet.identification(), Ok(0x1234));
    }

    #[test]
    fn test_check_len_rejects_invalid_ihl() {
        let mut bytes = PACKET_BYTES;
        bytes[0] = 0x41;
        let packet = Packet::new_unchecked(bytes);
        assert_eq!(packet.check_len(), Err(NetworkError::Invalid));
    }

    #[test]
    fn test_check_len_rejects_total_len_too_small() {
        let mut bytes = PACKET_BYTES;
        bytes[2] = 0x00;
        bytes[3] = 0x10;
        let packet = Packet::new_unchecked(bytes);
        assert_eq!(packet.check_len(), Err(NetworkError::Invalid));
    }

    #[test]
    fn test_parse_unsupported_protocol() {
        let mut bytes = PACKET_BYTES;
        bytes[9] = 250;
        let packet = Packet::new_unchecked(&bytes[..]);
        assert_eq!(Repr::parse(&packet), Err(NetworkError::Unsupported));
    }
}
