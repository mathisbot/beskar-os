use crate::{
    NetworkError, NetworkResult,
    utils::{checksum, u16_from_inet_bytes, u16_to_inet_bytes},
};

/// Range of bytes for the type field.
const TYPE: core::ops::Range<usize> = 0..1;
/// Range of bytes for the code field.
const CODE: core::ops::Range<usize> = 1..2;
/// Range of bytes for the checksum field.
const CHECKSUM: core::ops::Range<usize> = 2..4;
/// Range of bytes for the rest of header field.
const REST_OF_HEADER: core::ops::Range<usize> = 4..8;
/// Length of the ICMP header (fixed).
const HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
/// ICMP message type.
pub enum MessageType {
    EchoReply = 0,
    DestinationUnreachable = 3,
    SourceQuench = 4,
    Redirect = 5,
    EchoRequest = 8,
    RouterAdvertisement = 9,
    RouterSolicitation = 10,
    TimeExceeded = 11,
    ParameterProblem = 12,
    Timestamp = 13,
    TimestampReply = 14,
    InformationRequest = 15,
    InformationReply = 16,
}

impl TryFrom<u8> for MessageType {
    type Error = NetworkError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::EchoReply),
            3 => Ok(Self::DestinationUnreachable),
            4 => Ok(Self::SourceQuench),
            5 => Ok(Self::Redirect),
            8 => Ok(Self::EchoRequest),
            9 => Ok(Self::RouterAdvertisement),
            10 => Ok(Self::RouterSolicitation),
            11 => Ok(Self::TimeExceeded),
            12 => Ok(Self::ParameterProblem),
            13 => Ok(Self::Timestamp),
            14 => Ok(Self::TimestampReply),
            15 => Ok(Self::InformationRequest),
            16 => Ok(Self::InformationReply),
            _ => Err(NetworkError::Invalid),
        }
    }
}

impl From<MessageType> for u8 {
    fn from(value: MessageType) -> Self {
        value as Self
    }
}

/// A read/write wrapper around an ICMP packet buffer.
#[derive(Debug, Clone)]
pub struct Packet<T: AsRef<[u8]>> {
    buffer: T,
}

impl<T: AsRef<[u8]>> Packet<T> {
    #[must_use]
    #[inline]
    /// Imbue a raw octet buffer with ICMP packet structure.
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
    /// Return the message type field.
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    pub fn msg_type(&self) -> MessageType {
        MessageType::try_from(self.buffer.as_ref()[TYPE.start]).unwrap()
    }

    #[must_use]
    #[inline]
    /// Return the code field.
    pub fn code(&self) -> u8 {
        self.buffer.as_ref()[CODE.start]
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
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the rest of header field.
    pub fn rest_of_header(&self) -> u32 {
        let data = self.buffer.as_ref();
        u32::from_be_bytes(data[REST_OF_HEADER].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return identifier and sequence number for echo messages.
    /// Returns `(identifier, sequence)` as a tuple.
    pub fn echo_identity(&self) -> (u16, u16) {
        let data = self.buffer.as_ref();
        let ident = u16_from_inet_bytes(data[4..6].try_into().unwrap());
        let seq = u16_from_inet_bytes(data[6..8].try_into().unwrap());
        (ident, seq)
    }

    #[must_use]
    #[inline]
    /// Return the payload (data after the header).
    pub fn payload(&self) -> &[u8] {
        let data = self.buffer.as_ref();
        &data[HEADER_LEN..]
    }

    #[must_use]
    #[inline]
    /// Return the length of the ICMP header.
    pub const fn header_len() -> usize {
        HEADER_LEN
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    #[inline]
    /// Set the message type field.
    pub fn set_msg_type(&mut self, value: MessageType) {
        self.buffer.as_mut()[TYPE.start] = value.into();
    }

    #[inline]
    /// Set the code field.
    pub fn set_code(&mut self, value: u8) {
        self.buffer.as_mut()[CODE.start] = value;
    }

    #[inline]
    /// Set the checksum field.
    pub fn set_checksum(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[CHECKSUM].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the rest of header field.
    pub fn set_rest_of_header(&mut self, value: u32) {
        let data = self.buffer.as_mut();
        data[REST_OF_HEADER].copy_from_slice(&value.to_be_bytes());
    }

    /// Set identifier and sequence number for echo messages.
    #[inline]
    pub fn set_echo_identity(&mut self, ident: u16, seq: u16) {
        let data = self.buffer.as_mut();
        data[4..6].copy_from_slice(&u16_to_inet_bytes(ident));
        data[6..8].copy_from_slice(&u16_to_inet_bytes(seq));
    }

    #[inline]
    /// Get a mutable reference to the payload.
    pub fn payload_mut(&mut self) -> &mut [u8] {
        let data = self.buffer.as_mut();
        &mut data[HEADER_LEN..]
    }

    /// Recalculate and set the checksum.
    pub fn fill_checksum(&mut self) {
        self.set_checksum(0);
        let data = self.buffer.as_ref();
        let cksum = checksum(data);
        let _ = data;
        self.set_checksum(cksum);
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for Packet<T> {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

/// A high-level representation of an ICMP packet.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Repr {
    pub msg_type: MessageType,
    pub code: u8,
    pub payload_len: usize,
}

impl Repr {
    #[inline]
    /// Parse an ICMP packet and return a high-level representation.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the packet is too short.
    pub fn parse<T: AsRef<[u8]> + ?Sized>(packet: &Packet<&T>) -> NetworkResult<Self> {
        packet.check_len()?;

        Ok(Self {
            msg_type: packet.msg_type(),
            code: packet.code(),
            payload_len: packet.payload().len(),
        })
    }

    #[must_use]
    #[inline]
    /// Return the length of a packet that will be emitted from this high-level representation.
    pub const fn buffer_len(&self) -> usize {
        HEADER_LEN + self.payload_len
    }

    /// Emit a high-level representation into an ICMP packet.
    ///
    /// # Panics
    ///
    /// Panics if the packet buffer is too short.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) {
        assert!(packet.buffer.as_ref().len() >= self.buffer_len());
        packet.set_msg_type(self.msg_type);
        packet.set_code(self.code);
        packet.set_rest_of_header(0);
        packet.fill_checksum();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec;

    static ECHO_REQUEST: [u8; 8] = [0x08, 0x00, 0xf7, 0xff, 0x00, 0x01, 0x00, 0x02];

    #[test]
    fn test_echo_request() {
        let packet = Packet::new_unchecked(&ECHO_REQUEST[..]);
        assert_eq!(packet.msg_type(), MessageType::EchoRequest);
        assert_eq!(packet.code(), 0);
        let (ident, seq) = packet.echo_identity();
        assert_eq!(ident, 1);
        assert_eq!(seq, 2);
    }

    #[test]
    fn test_construct_echo_reply() {
        let mut bytes = vec![0u8; 8];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_msg_type(MessageType::EchoReply);
        packet.set_code(0);
        packet.set_echo_identity(1, 2);
        packet.fill_checksum();

        assert_eq!(packet.msg_type(), MessageType::EchoReply);
        assert_eq!(packet.code(), 0);
        let (ident, seq) = packet.echo_identity();
        assert_eq!(ident, 1);
        assert_eq!(seq, 2);
    }
}
