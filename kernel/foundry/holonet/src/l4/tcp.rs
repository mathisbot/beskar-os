pub use core::net::{SocketAddr, SocketAddrV4, SocketAddrV6};

use crate::{
    NetworkError, NetworkResult,
    l3::ip::Ipv4Addr,
    utils::{
        checksum_with_pseudo, u16_from_inet_bytes, u16_to_inet_bytes, u32_from_inet_bytes,
        u32_to_inet_bytes,
    },
};

/// Range of bytes for the source port field.
const SOURCE_PORT: core::ops::Range<usize> = 0..2;
/// Range of bytes for the destination port field.
const DEST_PORT: core::ops::Range<usize> = 2..4;
/// Range of bytes for the sequence number field.
const SEQUENCE_NUM: core::ops::Range<usize> = 4..8;
/// Range of bytes for the acknowledgment number field.
const ACK_NUM: core::ops::Range<usize> = 8..12;
/// Range of bytes for the data offset and flags field.
const DATA_OFFSET_AND_FLAGS: core::ops::Range<usize> = 12..14;
/// Range of bytes for the window size field.
const WINDOW_SIZE: core::ops::Range<usize> = 14..16;
/// Range of bytes for the checksum field.
const CHECKSUM: core::ops::Range<usize> = 16..18;
/// Range of bytes for the urgent pointer field.
const URGENT_PTR: core::ops::Range<usize> = 18..20;
/// Length of the TCP header (minimum).
const HEADER_LEN: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// TCP packet flags.
pub struct Flags(u8);

impl Flags {
    pub const FIN: u8 = 0x01;
    pub const SYN: u8 = 0x02;
    pub const RST: u8 = 0x04;
    pub const PSH: u8 = 0x08;
    pub const ACK: u8 = 0x10;
    pub const URG: u8 = 0x20;

    #[must_use]
    #[inline]
    /// Parse flags from the byte.
    pub const fn from_bits(value: u8) -> Self {
        Self(value & 0x3F)
    }

    #[must_use]
    #[inline]
    /// Convert flags to byte representation.
    pub const fn to_bits(self) -> u8 {
        self.0
    }

    #[must_use]
    #[inline]
    pub const fn fin(&self) -> bool {
        (self.0 & Self::FIN) != 0
    }

    #[must_use]
    #[inline]
    pub const fn syn(&self) -> bool {
        (self.0 & Self::SYN) != 0
    }

    #[must_use]
    #[inline]
    pub const fn rst(&self) -> bool {
        (self.0 & Self::RST) != 0
    }

    #[must_use]
    #[inline]
    pub const fn psh(&self) -> bool {
        (self.0 & Self::PSH) != 0
    }

    #[must_use]
    #[inline]
    pub const fn ack(&self) -> bool {
        (self.0 & Self::ACK) != 0
    }

    #[must_use]
    #[inline]
    pub const fn urg(&self) -> bool {
        (self.0 & Self::URG) != 0
    }
}

/// A read/write wrapper around a TCP packet buffer.
#[derive(Debug, Clone)]
pub struct Packet<T: AsRef<[u8]>> {
    buffer: T,
}

impl<T: AsRef<[u8]>> Packet<T> {
    #[must_use]
    #[inline]
    /// Imbue a raw octet buffer with TCP packet structure.
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
    /// Return the sequence number field.
    pub fn seq_num(&self) -> u32 {
        let data = self.buffer.as_ref();
        u32_from_inet_bytes(data[SEQUENCE_NUM].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the acknowledgment number field.
    pub fn ack_num(&self) -> u32 {
        let data = self.buffer.as_ref();
        u32_from_inet_bytes(data[ACK_NUM].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    /// Return the data offset (in 32-bit words).
    pub fn data_offset(&self) -> u8 {
        self.buffer.as_ref()[DATA_OFFSET_AND_FLAGS.start] >> 4
    }

    #[must_use]
    #[inline]
    /// Return the header length in bytes.
    pub fn header_len(&self) -> usize {
        (self.data_offset() as usize) * 4
    }

    #[must_use]
    #[inline]
    /// Return the TCP flags.
    pub fn flags(&self) -> Flags {
        let data = self.buffer.as_ref();
        Flags::from_bits(data[DATA_OFFSET_AND_FLAGS.start + 1])
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the window size field.
    pub fn window_size(&self) -> u16 {
        let data = self.buffer.as_ref();
        u16_from_inet_bytes(data[WINDOW_SIZE].try_into().unwrap())
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
    /// Return the urgent pointer field.
    pub fn urgent_ptr(&self) -> u16 {
        let data = self.buffer.as_ref();
        u16_from_inet_bytes(data[URGENT_PTR].try_into().unwrap())
    }

    #[must_use]
    #[inline]
    /// Return the payload.
    pub fn payload(&self) -> &[u8] {
        let data = self.buffer.as_ref();
        &data[self.header_len()..]
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains a SYN flag.
    pub fn is_syn(&self) -> bool {
        self.flags().syn()
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains an ACK flag.
    pub fn is_ack(&self) -> bool {
        self.flags().ack()
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains an FIN flag.
    pub fn is_fin(&self) -> bool {
        self.flags().fin()
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains an RST flag.
    pub fn is_rst(&self) -> bool {
        self.flags().rst()
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains a PSH flag.
    pub fn is_psh(&self) -> bool {
        self.flags().psh()
    }

    #[must_use]
    #[inline]
    /// Return whether the packet is empty (only contains the header).
    pub fn is_empty(&self) -> bool {
        self.payload().is_empty()
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
    /// Set the sequence number field.
    pub fn set_seq_num(&mut self, value: u32) {
        let data = self.buffer.as_mut();
        data[SEQUENCE_NUM].copy_from_slice(&u32_to_inet_bytes(value));
    }

    #[inline]
    /// Set the acknowledgment number field.
    pub fn set_ack_num(&mut self, value: u32) {
        let data = self.buffer.as_mut();
        data[ACK_NUM].copy_from_slice(&u32_to_inet_bytes(value));
    }

    #[inline]
    /// Set the data offset (in 32-bit words).
    pub fn set_data_offset(&mut self, value: u8) {
        self.buffer.as_mut()[DATA_OFFSET_AND_FLAGS.start] = (value & 0x0F) << 4;
    }

    #[inline]
    /// Set the TCP flags.
    pub fn set_flags(&mut self, flags: Flags) {
        self.buffer.as_mut()[DATA_OFFSET_AND_FLAGS.start + 1] = flags.to_bits();
    }

    #[inline]
    /// Set the window size field.
    pub fn set_window_size(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[WINDOW_SIZE].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the checksum field.
    pub fn set_checksum(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[CHECKSUM].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Set the urgent pointer field.
    pub fn set_urgent_ptr(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        data[URGENT_PTR].copy_from_slice(&u16_to_inet_bytes(value));
    }

    #[inline]
    /// Get a mutable reference to the payload.
    pub fn payload_mut(&mut self) -> &mut [u8] {
        let header_len = self.header_len();
        let data = self.buffer.as_mut();
        &mut data[header_len..]
    }

    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Recalculate and set the TCP checksum with pseudo-header (IPv4).
    pub fn fill_checksum(&mut self, src_addr: Ipv4Addr, dst_addr: Ipv4Addr) {
        // Build pseudo-header
        let mut pseudo = [0u8; 12];
        pseudo[0..4].copy_from_slice(&src_addr.octets());
        pseudo[4..8].copy_from_slice(&dst_addr.octets());
        pseudo[8] = 0; // Reserved
        pseudo[9] = 6; // Protocol (TCP)

        let total_len = self.header_len() + self.payload().len();
        pseudo[10..12].copy_from_slice(&u16_to_inet_bytes(u16::try_from(total_len).unwrap()));

        self.set_checksum(0);
        let data = self.buffer.as_ref();
        let cksum = checksum_with_pseudo(&pseudo, data);
        let _ = data;
        self.set_checksum(cksum);
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for Packet<T> {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

/// A high-level representation of a TCP packet.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Repr {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub flags: Flags,
    pub window_size: u16,
    pub payload_len: usize,
}

impl Repr {
    #[inline]
    /// Parse a TCP packet and return a high-level representation.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the packet is too short.
    pub fn parse<T: AsRef<[u8]> + ?Sized>(packet: &Packet<&T>) -> NetworkResult<Self> {
        packet.check_len()?;

        Ok(Self {
            src_port: packet.src_port(),
            dst_port: packet.dst_port(),
            seq_num: packet.seq_num(),
            ack_num: packet.ack_num(),
            flags: packet.flags(),
            window_size: packet.window_size(),
            payload_len: packet.payload().len(),
        })
    }

    #[must_use]
    #[inline]
    /// Return the length of a packet that will be emitted from this high-level representation.
    pub const fn buffer_len(&self) -> usize {
        HEADER_LEN + self.payload_len
    }

    /// Emit a high-level representation into a TCP packet.
    ///
    /// # Panics
    ///
    /// Panics if the packet buffer is too short.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) {
        assert!(packet.buffer.as_ref().len() >= self.buffer_len());
        packet.set_src_port(self.src_port);
        packet.set_dst_port(self.dst_port);
        packet.set_seq_num(self.seq_num);
        packet.set_ack_num(self.ack_num);
        packet.set_data_offset(5); // 5 * 4 = 20 bytes (minimum header)
        packet.set_flags(self.flags);
        packet.set_window_size(self.window_size);
        packet.set_urgent_ptr(0);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec;

    static PACKET_BYTES: [u8; 20] = [
        0x00, 0x50, // Source port 80
        0x12, 0x34, // Destination port 4660
        0x00, 0x00, 0x00, 0x01, // Sequence number
        0x00, 0x00, 0x00, 0x00, // Acknowledgment number
        0x50, 0x12, // Data offset 5, SYN+ACK flags
        0x20, 0x00, // Window size 8192
        0x00, 0x00, // Checksum
        0x00, 0x00, // Urgent pointer
    ];

    #[test]
    fn test_src_dst_port() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.src_port(), 80);
        assert_eq!(packet.dst_port(), 4660);
    }

    #[test]
    fn test_seq_ack_num() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.seq_num(), 1);
        assert_eq!(packet.ack_num(), 0);
    }

    #[test]
    fn test_data_offset() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.data_offset(), 5);
        assert_eq!(packet.header_len(), 20);
    }

    #[test]
    fn test_flags() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        let flags = packet.flags();
        assert!(!flags.fin());
        assert!(flags.syn());
        assert!(!flags.rst());
        assert!(!flags.psh());
        assert!(flags.ack());
        assert!(!flags.urg());
    }

    #[test]
    fn test_window_size() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.window_size(), 8192);
    }

    #[test]
    fn test_construct() {
        let mut bytes = vec![0u8; 20];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_src_port(80);
        packet.set_dst_port(4660);
        packet.set_seq_num(1);
        packet.set_ack_num(0);
        packet.set_data_offset(5);
        packet.set_window_size(8192);

        assert_eq!(packet.src_port(), 80);
        assert_eq!(packet.dst_port(), 4660);
        assert_eq!(packet.seq_num(), 1);
        assert_eq!(packet.window_size(), 8192);
    }
}
