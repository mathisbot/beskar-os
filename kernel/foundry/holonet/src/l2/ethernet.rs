use crate::{
    NetworkError, NetworkResult,
    utils::{ensure_len, read_array, read_u16, slice, slice_mut, write_slice, write_u16},
};

const DESTINATION: usize = 0;
const SOURCE: usize = 6;
const ETHERTYPE: usize = 12;
const PAYLOAD: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
#[non_exhaustive]
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
    pub const BROADCAST: Self = Self([0xFF; _]);

    #[must_use]
    #[inline]
    /// Construct an Ethernet address from a six-octet array.
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    #[must_use]
    /// # Errors
    ///
    /// Returns `Truncated` if `data` is not six bytes long.
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        let bytes: [u8; 6] = data.try_into().map_err(|_| NetworkError::Truncated)?;
        Ok(Self::new(bytes))
    }

    #[must_use]
    #[inline]
    /// Return an Ethernet address as a sequence of bytes, in big-endian.
    pub const fn as_bytes(&self) -> [u8; 6] {
        self.0
    }

    #[must_use]
    #[inline]
    /// Whether the address is a unicast address.
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
pub const HEADER_LEN: usize = PAYLOAD;

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
    /// Returns `Truncated` if the buffer is too short.
    pub fn new(buffer: T) -> NetworkResult<Self> {
        let packet = Self::new_unchecked(buffer);
        packet.check_len()?;
        Ok(packet)
    }

    /// # Errors
    ///
    /// Returns `Truncated` if the buffer is too short.
    pub fn check_len(&self) -> NetworkResult<()> {
        ensure_len(self.buffer.as_ref(), HEADER_LEN)
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
    pub fn dst_addr(&self) -> NetworkResult<MacAddress> {
        let data = self.buffer.as_ref();
        let raw = read_array(data, DESTINATION)?;
        Ok(MacAddress::new(raw))
    }

    #[must_use]
    #[inline]
    /// Return the source address field.
    pub fn src_addr(&self) -> NetworkResult<MacAddress> {
        let data = self.buffer.as_ref();
        let raw = read_array(data, SOURCE)?;
        Ok(MacAddress::new(raw))
    }

    #[must_use]
    #[inline]
    /// Return the raw `EtherType` field value.
    pub fn ethertype_raw(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), ETHERTYPE)
    }

    #[must_use]
    #[inline]
    /// Return the `EtherType` field, without checking for 802.1Q.
    /// Return the `EtherType` field, without checking for 802.1Q.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the value does not map to a known `EtherType`.
    pub fn ethertype(&self) -> NetworkResult<EtherType> {
        EtherType::try_from(self.ethertype_raw()?)
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Frame<&'a T> {
    #[must_use]
    #[inline]
    /// Return a pointer to the payload, without checking for IEEE 802.1Q.
    pub fn payload(&self) -> NetworkResult<&'a [u8]> {
        slice(self.buffer.as_ref(), PAYLOAD..)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Frame<T> {
    #[inline]
    /// Set the destination address field.
    pub fn set_dst_addr(&mut self, value: MacAddress) -> NetworkResult<()> {
        write_slice(self.buffer.as_mut(), DESTINATION, &value.as_bytes())
    }

    #[inline]
    /// Set the source address field.
    pub fn set_src_addr(&mut self, value: MacAddress) -> NetworkResult<()> {
        write_slice(self.buffer.as_mut(), SOURCE, &value.as_bytes())
    }

    #[inline]
    /// Set the `EtherType` field.
    pub fn set_ethertype(&mut self, value: EtherType) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), ETHERTYPE, value.into())
    }

    #[must_use]
    #[inline]
    /// Return a mutable pointer to the payload.
    pub fn payload_mut(&mut self) -> NetworkResult<&mut [u8]> {
        slice_mut(self.buffer.as_mut(), PAYLOAD..)
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
        Ok(Self {
            src_addr: frame.src_addr()?,
            dst_addr: frame.dst_addr()?,
            ethertype: frame.ethertype()?,
        })
    }

    #[must_use]
    #[inline]
    /// Return the length of a header that will be emitted from this high-level representation.
    pub const fn buffer_len(&self) -> usize {
        HEADER_LEN
    }

    /// # Errors
    ///
    /// Returns `Truncated` if the frame buffer is too short.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, frame: &mut Frame<T>) -> NetworkResult<()> {
        frame.set_src_addr(self.src_addr)?;
        frame.set_dst_addr(self.dst_addr)?;
        frame.set_ethertype(self.ethertype)
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
            Ok(MacAddress([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]))
        );
        assert_eq!(
            frame.src_addr(),
            Ok(MacAddress([0x11, 0x12, 0x13, 0x14, 0x15, 0x16]))
        );
        assert_eq!(frame.ethertype(), Ok(EtherType::IpV4));
        assert_eq!(frame.payload(), Ok(&PAYLOAD_BYTES_V4[..]));
    }

    #[test]
    fn test_v4_construct() {
        let mut bytes = vec![0xa5; 64];
        let mut frame = Frame::new_unchecked(&mut bytes);
        frame
            .set_dst_addr(MacAddress([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]))
            .unwrap();
        frame
            .set_src_addr(MacAddress([0x11, 0x12, 0x13, 0x14, 0x15, 0x16]))
            .unwrap();
        frame.set_ethertype(EtherType::IpV4).unwrap();
        frame
            .payload_mut()
            .unwrap()
            .copy_from_slice(&PAYLOAD_BYTES_V4[..]);
        assert_eq!(&frame.into_inner()[..], &FRAME_BYTES_V4[..]);
    }

    #[test]
    fn test_v6_deconstruct() {
        let frame = Frame::new_unchecked(&FRAME_BYTES_V6[..]);
        assert_eq!(
            frame.dst_addr(),
            Ok(MacAddress([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]))
        );
        assert_eq!(
            frame.src_addr(),
            Ok(MacAddress([0x11, 0x12, 0x13, 0x14, 0x15, 0x16]))
        );
        assert_eq!(frame.ethertype(), Ok(EtherType::IpV6));
        assert_eq!(frame.payload(), Ok(&PAYLOAD_BYTES_V6[..]));
    }

    #[test]
    fn test_v6_construct() {
        let mut bytes = vec![0xa5; 54];
        let mut frame = Frame::new_unchecked(&mut bytes);
        frame
            .set_dst_addr(MacAddress([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]))
            .unwrap();
        frame
            .set_src_addr(MacAddress([0x11, 0x12, 0x13, 0x14, 0x15, 0x16]))
            .unwrap();
        frame.set_ethertype(EtherType::IpV6).unwrap();
        assert_eq!(PAYLOAD_BYTES_V6.len(), frame.payload_mut().unwrap().len());
        frame
            .payload_mut()
            .unwrap()
            .copy_from_slice(&PAYLOAD_BYTES_V6[..]);
        assert_eq!(&frame.into_inner()[..], &FRAME_BYTES_V6[..]);
    }

    #[test]
    fn test_parse_unknown_ethertype_is_error() {
        let mut bytes = FRAME_BYTES_V4;
        bytes[12] = 0x12;
        bytes[13] = 0x34;
        let frame = Frame::new_unchecked(&bytes[..]);
        assert_eq!(Repr::parse(&frame), Err(NetworkError::Invalid));
    }
}
