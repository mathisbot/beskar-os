use crate::{
    NetworkError, NetworkResult,
    utils::{u16_from_inet_bytes, u16_to_inet_bytes},
};

const DESTINATION: core::ops::Range<usize> = 0..6;
const SOURCE: core::ops::Range<usize> = 6..12;
const ETHERTYPE: core::ops::Range<usize> = 12..14;
const PAYLOAD: core::ops::RangeFrom<usize> = 14..;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
/// Ethernet protocol type.
pub enum EtherType {
    IpV4 = 0x0800,
    Arp = 0x0806,
    IpV6 = 0x86DD,
}

impl TryFrom<u16> for EtherType {
    type Error = NetworkError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0800 => Ok(Self::IpV4),
            0x0806 => Ok(Self::Arp),
            0x86DD => Ok(Self::IpV6),
            _ => Err(NetworkError::Invalid),
        }
    }
}

impl From<EtherType> for u16 {
    fn from(value: EtherType) -> Self {
        value as Self
    }
}

/// A six-octet Ethernet II address.
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    /// The broadcast address.
    pub const BROADCAST: Self = Self([0xff; 6]);

    #[must_use]
    #[inline]
    /// Construct an Ethernet address from a six-octet array.
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    #[must_use]
    /// # Panics
    ///
    /// The function panics if `data` is not six octets long.
    pub const fn from_bytes(data: &[u8]) -> Self {
        let mut bytes = [0; 6];
        bytes.copy_from_slice(data);
        Self::new(bytes)
    }

    #[must_use]
    #[inline]
    /// Return an Ethernet address as a sequence of octets, in big-endian.
    pub const fn as_bytes(&self) -> [u8; 6] {
        self.0
    }

    #[must_use]
    #[inline]
    /// Whether the address is an unicast address.
    pub fn is_unicast(&self) -> bool {
        !(self.is_broadcast() || self.is_multicast())
    }

    #[must_use]
    #[inline]
    /// Whether this address is the broadcast address.
    pub fn is_broadcast(&self) -> bool {
        *self == Self::BROADCAST
    }

    #[must_use]
    #[inline]
    /// Whether the "multicast" bit in the OUI is set.
    pub const fn is_multicast(&self) -> bool {
        self.0[0] & 0x01 != 0
    }

    #[must_use]
    #[inline]
    /// Whether the "locally administered" bit in the OUI is set.
    pub const fn is_local(&self) -> bool {
        self.0[0] & 0x02 != 0
    }
}

impl core::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let bytes = self.0;
        write!(
            f,
            "{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}",
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
        )
    }
}

/// A read/write wrapper around an Ethernet II frame buffer.
#[derive(Debug, Clone)]
pub struct Frame<T: AsRef<[u8]>> {
    buffer: T,
}

/// The Ethernet header length
pub const HEADER_LEN: usize = PAYLOAD.start;

impl<T: AsRef<[u8]>> Frame<T> {
    #[must_use]
    #[inline]
    /// Imbue a raw octet buffer with Ethernet frame structure.
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
    /// Consumes the frame, returning the underlying buffer.
    pub fn into_inner(self) -> T {
        self.buffer
    }

    #[must_use]
    #[inline]
    /// Return the length of a frame header.
    pub const fn header_len() -> usize {
        HEADER_LEN
    }

    #[must_use]
    #[inline]
    /// Return the length of a buffer required to hold a packet with the payload
    /// of a given length.
    pub const fn buffer_len(payload_len: usize) -> usize {
        HEADER_LEN + payload_len
    }

    #[must_use]
    #[inline]
    /// Return the destination address field.
    pub fn dst_addr(&self) -> MacAddress {
        let data = self.buffer.as_ref();
        MacAddress::from_bytes(&data[DESTINATION])
    }

    #[must_use]
    #[inline]
    /// Return the source address field.
    pub fn src_addr(&self) -> MacAddress {
        let data = self.buffer.as_ref();
        MacAddress::from_bytes(&data[SOURCE])
    }

    #[must_use]
    #[inline]
    #[allow(clippy::missing_panics_doc)] // Never panics!
    /// Return the `EtherType` field, without checking for 802.1Q.
    pub fn ethertype(&self) -> EtherType {
        let data = self.buffer.as_ref();
        let raw = u16_from_inet_bytes(data[ETHERTYPE].try_into().unwrap());
        EtherType::try_from(raw).unwrap()
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Frame<&'a T> {
    #[must_use]
    #[inline]
    /// Return a pointer to the payload, without checking for IEEE 802.1Q.
    pub fn payload(&self) -> &'a [u8] {
        let data = self.buffer.as_ref();
        &data[PAYLOAD]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Frame<T> {
    #[inline]
    /// Set the destination address field.
    pub fn set_dst_addr(&mut self, value: MacAddress) {
        let data = self.buffer.as_mut();
        data[DESTINATION].copy_from_slice(&value.as_bytes());
    }

    #[inline]
    /// Set the source address field.
    pub fn set_src_addr(&mut self, value: MacAddress) {
        let data = self.buffer.as_mut();
        data[SOURCE].copy_from_slice(&value.as_bytes());
    }

    #[inline]
    /// Set the `EtherType` field.
    pub fn set_ethertype(&mut self, value: EtherType) {
        let data = self.buffer.as_mut();
        data[ETHERTYPE].copy_from_slice(&u16_to_inet_bytes(value.into()));
    }

    #[must_use]
    #[inline]
    /// Return a mutable pointer to the payload.
    pub fn payload_mut(&mut self) -> &mut [u8] {
        let data = self.buffer.as_mut();
        &mut data[PAYLOAD]
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for Frame<T> {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

/// A high-level representation of an Internet Protocol version 4 packet header.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Repr {
    pub src_addr: MacAddress,
    pub dst_addr: MacAddress,
    pub ethertype: EtherType,
}

impl Repr {
    #[inline]
    /// Parse an Ethernet II frame and return a high-level representation.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the frame is too short.
    pub fn parse<T: AsRef<[u8]> + ?Sized>(frame: &Frame<&T>) -> NetworkResult<Self> {
        frame.check_len()?;
        Ok(Self {
            src_addr: frame.src_addr(),
            dst_addr: frame.dst_addr(),
            ethertype: frame.ethertype(),
        })
    }

    #[must_use]
    #[inline]
    /// Return the length of a header that will be emitted from this high-level representation.
    pub const fn buffer_len(&self) -> usize {
        HEADER_LEN
    }

    /// # Panics
    ///
    /// Panics if the frame buffer is too short.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, frame: &mut Frame<T>) {
        assert!(frame.buffer.as_ref().len() >= self.buffer_len());
        frame.set_src_addr(self.src_addr);
        frame.set_dst_addr(self.dst_addr);
        frame.set_ethertype(self.ethertype);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec;

    static FRAME_BYTES_V4: [u8; 64] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x08, 0x00, 0xaa,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0xff,
    ];

    static PAYLOAD_BYTES_V4: [u8; 50] = [
        0xaa, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff,
    ];

    static FRAME_BYTES_V6: [u8; 54] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x86, 0xdd, 0x60,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xfe, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    ];

    static PAYLOAD_BYTES_V6: [u8; 40] = [
        0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xfe, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    ];

    #[test]
    fn test_broadcast() {
        assert!(MacAddress::BROADCAST.is_broadcast());
        assert!(!MacAddress::BROADCAST.is_unicast());
        assert!(MacAddress::BROADCAST.is_multicast());
        assert!(MacAddress::BROADCAST.is_local());
    }

    #[test]
    fn test_v4_deconstruct() {
        let frame = Frame::new_unchecked(&FRAME_BYTES_V4[..]);
        assert_eq!(
            frame.dst_addr(),
            MacAddress([0x01, 0x02, 0x03, 0x04, 0x05, 0x06])
        );
        assert_eq!(
            frame.src_addr(),
            MacAddress([0x11, 0x12, 0x13, 0x14, 0x15, 0x16])
        );
        assert_eq!(frame.ethertype(), EtherType::IpV4);
        assert_eq!(frame.payload(), &PAYLOAD_BYTES_V4[..]);
    }

    #[test]
    fn test_v4_construct() {
        let mut bytes = vec![0xa5; 64];
        let mut frame = Frame::new_unchecked(&mut bytes);
        frame.set_dst_addr(MacAddress([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]));
        frame.set_src_addr(MacAddress([0x11, 0x12, 0x13, 0x14, 0x15, 0x16]));
        frame.set_ethertype(EtherType::IpV4);
        frame.payload_mut().copy_from_slice(&PAYLOAD_BYTES_V4[..]);
        assert_eq!(&frame.into_inner()[..], &FRAME_BYTES_V4[..]);
    }

    #[test]
    fn test_v6_deconstruct() {
        let frame = Frame::new_unchecked(&FRAME_BYTES_V6[..]);
        assert_eq!(
            frame.dst_addr(),
            MacAddress([0x01, 0x02, 0x03, 0x04, 0x05, 0x06])
        );
        assert_eq!(
            frame.src_addr(),
            MacAddress([0x11, 0x12, 0x13, 0x14, 0x15, 0x16])
        );
        assert_eq!(frame.ethertype(), EtherType::IpV6);
        assert_eq!(frame.payload(), &PAYLOAD_BYTES_V6[..]);
    }

    #[test]
    fn test_v6_construct() {
        let mut bytes = vec![0xa5; 54];
        let mut frame = Frame::new_unchecked(&mut bytes);
        frame.set_dst_addr(MacAddress([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]));
        frame.set_src_addr(MacAddress([0x11, 0x12, 0x13, 0x14, 0x15, 0x16]));
        frame.set_ethertype(EtherType::IpV6);
        assert_eq!(PAYLOAD_BYTES_V6.len(), frame.payload_mut().len());
        frame.payload_mut().copy_from_slice(&PAYLOAD_BYTES_V6[..]);
        assert_eq!(&frame.into_inner()[..], &FRAME_BYTES_V6[..]);
    }
}
