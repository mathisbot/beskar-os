pub use core::net::{SocketAddr, SocketAddrV4, SocketAddrV6};

use crate::{
    NetworkError, NetworkResult,
    egress::EthernetIpv4Envelope,
    l3::ip::v4::Ipv4Addr,
    utils::{
        checksum_with_pseudo, ensure_len, read_u8, read_u16, read_u32, slice, slice_mut,
        usize_to_u16, write_u8, write_u16, write_u32,
    },
};

/// Range of bytes for the source port field.
const SOURCE_PORT: usize = 0;
/// Range of bytes for the destination port field.
const DEST_PORT: usize = 2;
/// Range of bytes for the sequence number field.
const SEQUENCE_NUM: usize = 4;
/// Range of bytes for the acknowledgment number field.
const ACK_NUM: usize = 8;
/// Range of bytes for the data offset and flags field.
const DATA_OFFSET_AND_FLAGS: usize = 12;
/// Range of bytes for the window size field.
const WINDOW_SIZE: usize = 14;
/// Range of bytes for the checksum field.
const CHECKSUM: usize = 16;
/// Range of bytes for the urgent pointer field.
const URGENT_PTR: usize = 18;
/// Length of the TCP header (minimum).
pub const HEADER_LEN: usize = 20;

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
    pub const fn new(raw: u8) -> Self {
        Self::from_bits(raw)
    }

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

/// Borrowed TCP segment metadata and payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment<'a> {
    pub source_addr: Ipv4Addr,
    pub destination_addr: Ipv4Addr,
    pub source_port: u16,
    pub destination_port: u16,
    pub sequence_number: u32,
    pub acknowledgment_number: u32,
    pub flags: Flags,
    pub window_size: u16,
    pub payload: &'a [u8],
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
    /// Returns `Invalid` if the buffer is too short or if the header length field is invalid.
    pub fn check_len(&self) -> NetworkResult<()> {
        let data = self.buffer.as_ref();
        ensure_len(data, HEADER_LEN)?;

        let data_offset = data[DATA_OFFSET_AND_FLAGS] >> 4;
        if data_offset < 5 {
            return Err(NetworkError::Invalid);
        }

        let header_len = usize::from(data_offset) * 4;
        ensure_len(data, header_len)?;

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
    /// Return the source port field.
    pub fn src_port(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), SOURCE_PORT)
    }

    #[must_use]
    #[inline]
    /// Return the destination port field.
    pub fn dst_port(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), DEST_PORT)
    }

    #[must_use]
    #[inline]
    /// Return the sequence number field.
    pub fn seq_num(&self) -> NetworkResult<u32> {
        read_u32(self.buffer.as_ref(), SEQUENCE_NUM)
    }

    #[must_use]
    #[inline]
    /// Return the acknowledgment number field.
    pub fn ack_num(&self) -> NetworkResult<u32> {
        read_u32(self.buffer.as_ref(), ACK_NUM)
    }

    #[must_use]
    #[inline]
    /// Return the data offset (in 32-bit words).
    pub fn data_offset(&self) -> NetworkResult<u8> {
        Ok(read_u8(self.buffer.as_ref(), DATA_OFFSET_AND_FLAGS)? >> 4)
    }

    #[must_use]
    #[inline]
    /// Return the header length in bytes.
    pub fn header_len(&self) -> NetworkResult<usize> {
        Ok(usize::from(self.data_offset()?) * 4)
    }

    #[must_use]
    #[inline]
    /// Return the TCP flags.
    pub fn flags(&self) -> NetworkResult<Flags> {
        Ok(Flags::from_bits(read_u8(
            self.buffer.as_ref(),
            DATA_OFFSET_AND_FLAGS + 1,
        )?))
    }

    #[must_use]
    #[inline]
    /// Return the window size field.
    pub fn window_size(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), WINDOW_SIZE)
    }

    #[must_use]
    #[inline]
    /// Return the checksum field.
    pub fn checksum(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), CHECKSUM)
    }

    #[must_use]
    #[inline]
    /// Return the urgent pointer field.
    pub fn urgent_ptr(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), URGENT_PTR)
    }

    #[must_use]
    #[inline]
    /// Return the payload.
    pub fn payload(&self) -> NetworkResult<&[u8]> {
        let header_len = self.header_len()?;
        slice(self.buffer.as_ref(), header_len..)
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains a SYN flag.
    pub fn is_syn(&self) -> NetworkResult<bool> {
        Ok(self.flags()?.syn())
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains an ACK flag.
    pub fn is_ack(&self) -> NetworkResult<bool> {
        Ok(self.flags()?.ack())
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains an FIN flag.
    pub fn is_fin(&self) -> NetworkResult<bool> {
        Ok(self.flags()?.fin())
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains an RST flag.
    pub fn is_rst(&self) -> NetworkResult<bool> {
        Ok(self.flags()?.rst())
    }

    #[must_use]
    #[inline]
    /// Return whether the packet contains a PSH flag.
    pub fn is_psh(&self) -> NetworkResult<bool> {
        Ok(self.flags()?.psh())
    }

    #[must_use]
    #[inline]
    /// Return whether the packet is empty (only contains the header).
    pub fn is_empty(&self) -> NetworkResult<bool> {
        Ok(self.payload()?.is_empty())
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    #[inline]
    /// Set the source port field.
    pub fn set_src_port(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), SOURCE_PORT, value)
    }

    #[inline]
    /// Set the destination port field.
    pub fn set_dst_port(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), DEST_PORT, value)
    }

    #[inline]
    /// Set the sequence number field.
    pub fn set_seq_num(&mut self, value: u32) -> NetworkResult<()> {
        write_u32(self.buffer.as_mut(), SEQUENCE_NUM, value)
    }

    #[inline]
    /// Set the acknowledgment number field.
    pub fn set_ack_num(&mut self, value: u32) -> NetworkResult<()> {
        write_u32(self.buffer.as_mut(), ACK_NUM, value)
    }

    #[inline]
    /// Set the data offset (in 32-bit words).
    pub fn set_data_offset(&mut self, value: u8) -> NetworkResult<()> {
        write_u8(
            self.buffer.as_mut(),
            DATA_OFFSET_AND_FLAGS,
            (value & 0x0F) << 4,
        )
    }

    #[inline]
    /// Set the TCP flags.
    pub fn set_flags(&mut self, flags: Flags) -> NetworkResult<()> {
        write_u8(
            self.buffer.as_mut(),
            DATA_OFFSET_AND_FLAGS + 1,
            flags.to_bits(),
        )
    }

    #[inline]
    /// Set the window size field.
    pub fn set_window_size(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), WINDOW_SIZE, value)
    }

    #[inline]
    /// Set the checksum field.
    pub fn set_checksum(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), CHECKSUM, value)
    }

    #[inline]
    /// Set the urgent pointer field.
    pub fn set_urgent_ptr(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), URGENT_PTR, value)
    }

    #[inline]
    /// Get a mutable reference to the payload.
    pub fn payload_mut(&mut self) -> NetworkResult<&mut [u8]> {
        let header_len = self.header_len()?;
        ensure_len(self.buffer.as_ref(), header_len)?;
        slice_mut(self.buffer.as_mut(), header_len..)
    }

    /// Recalculate and set the TCP checksum with pseudo-header (IPv4).
    pub fn fill_checksum(&mut self, src_addr: Ipv4Addr, dst_addr: Ipv4Addr) -> NetworkResult<()> {
        // Build pseudo-header
        let mut pseudo = [0u8; 12];
        pseudo[0..4].copy_from_slice(&src_addr.octets());
        pseudo[4..8].copy_from_slice(&dst_addr.octets());
        pseudo[8] = 0; // Reserved
        pseudo[9] = 6; // Protocol (TCP)

        let total_len = self.buffer.as_ref().len();
        pseudo[10..12].copy_from_slice(&usize_to_u16(total_len)?.to_be_bytes());

        self.set_checksum(0)?;
        let data = self.buffer.as_ref();
        let cksum = checksum_with_pseudo(&pseudo, data);
        self.set_checksum(cksum)
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
            src_port: packet.src_port()?,
            dst_port: packet.dst_port()?,
            seq_num: packet.seq_num()?,
            ack_num: packet.ack_num()?,
            flags: packet.flags()?,
            window_size: packet.window_size()?,
            payload_len: packet.payload()?.len(),
        })
    }

    #[must_use]
    #[inline]
    /// Return the length of a packet that will be emitted from this high-level representation.
    pub const fn buffer_len(&self) -> usize {
        HEADER_LEN + self.payload_len
    }

    #[inline]
    pub fn packet_len(&self) -> NetworkResult<usize> {
        self.payload_len
            .checked_add(HEADER_LEN)
            .ok_or(NetworkError::Oversized)
    }

    /// Emit a high-level representation into a TCP packet.
    ///
    /// # Errors
    ///
    /// Returns `Truncated` if the packet buffer is too short.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) -> NetworkResult<()> {
        ensure_len(packet.buffer.as_ref(), self.packet_len()?)?;
        packet.set_src_port(self.src_port)?;
        packet.set_dst_port(self.dst_port)?;
        packet.set_seq_num(self.seq_num)?;
        packet.set_ack_num(self.ack_num)?;
        packet.set_data_offset(5)?; // 5 * 4 = 20 bytes (minimum header)
        packet.set_flags(self.flags)?;
        packet.set_window_size(self.window_size)?;
        packet.set_urgent_ptr(0)
    }
}

/// Emit a TCP segment inside an Ethernet + IPv4 frame.
///
/// `destination_hardware_addr` is the resolved next-hop Ethernet address.
#[allow(clippy::too_many_arguments)]
pub fn emit_ipv4(
    mut envelope: EthernetIpv4Envelope,
    buffer: &mut [u8],
    source_port: u16,
    destination_port: u16,
    sequence_number: u32,
    acknowledgment_number: u32,
    flags: Flags,
    window_size: u16,
    payload: &[u8],
) -> NetworkResult<usize> {
    let tcp_len = payload
        .len()
        .checked_add(HEADER_LEN)
        .ok_or(NetworkError::Oversized)?;
    envelope.payload_len = tcp_len;
    envelope.emit_with(buffer, |payload_buffer| {
        let mut packet = Packet::new_unchecked(payload_buffer);
        Repr {
            src_port: source_port,
            dst_port: destination_port,
            seq_num: sequence_number,
            ack_num: acknowledgment_number,
            flags,
            window_size,
            payload_len: payload.len(),
        }
        .emit(&mut packet)?;
        packet.payload_mut()?.copy_from_slice(payload);
        packet.fill_checksum(envelope.source_addr, envelope.destination_addr)
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        ingress::{EthernetPayload, IngressFrame, Ipv4Payload},
        l2::ethernet::{self, MacAddress},
        l3::ip,
    };
    use alloc::vec;

    const LOCAL_MAC: MacAddress = MacAddress::new([0x10, 0x11, 0x12, 0x13, 0x14, 0x15]);
    const PEER_MAC: MacAddress = MacAddress::new([0x20, 0x21, 0x22, 0x23, 0x24, 0x25]);
    const LOCAL_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 10);
    const REMOTE_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 1);

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
        assert_eq!(packet.src_port(), Ok(80));
        assert_eq!(packet.dst_port(), Ok(4660));
    }

    #[test]
    fn test_seq_ack_num() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.seq_num(), Ok(1));
        assert_eq!(packet.ack_num(), Ok(0));
    }

    #[test]
    fn test_data_offset() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.data_offset(), Ok(5));
        assert_eq!(packet.header_len(), Ok(20));
    }

    #[test]
    fn test_flags() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        let flags = packet.flags().unwrap();
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
        assert_eq!(packet.window_size(), Ok(8192));
    }

    #[test]
    fn test_construct() {
        let mut bytes = vec![0u8; 20];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_src_port(80).unwrap();
        packet.set_dst_port(4660).unwrap();
        packet.set_seq_num(1).unwrap();
        packet.set_ack_num(0).unwrap();
        packet.set_data_offset(5).unwrap();
        packet.set_window_size(8192).unwrap();

        assert_eq!(packet.src_port(), Ok(80));
        assert_eq!(packet.dst_port(), Ok(4660));
        assert_eq!(packet.seq_num(), Ok(1));
        assert_eq!(packet.window_size(), Ok(8192));
    }

    #[test]
    fn test_check_len_rejects_invalid_data_offset() {
        let mut bytes = PACKET_BYTES;
        bytes[12] = 0x40;
        let packet = Packet::new_unchecked(bytes);
        assert_eq!(packet.check_len(), Err(NetworkError::Invalid));
    }

    #[test]
    fn test_emit_ipv4_tcp_segment() {
        let payload = [1, 2, 3, 4];
        let mut bytes = [0u8; ethernet::HEADER_LEN + ip::v4::HEADER_LEN + HEADER_LEN + 4];

        let envelope = EthernetIpv4Envelope {
            source_hardware_addr: LOCAL_MAC,
            destination_hardware_addr: PEER_MAC,
            source_addr: LOCAL_IP,
            destination_addr: REMOTE_IP,
            protocol: ip::Protocol::Tcp,
            payload_len: 0, // will be set by emit_ipv4
            ttl: 64,
            flags: ip::v4::Flags::new(true, false),
        };

        let len = emit_ipv4(
            envelope,
            &mut bytes,
            1234,
            80,
            10,
            20,
            Flags::new(Flags::ACK | Flags::PSH),
            4096,
            &payload,
        )
        .unwrap();

        assert_eq!(len, bytes.len());

        let frame = IngressFrame::parse(&bytes).unwrap();
        match frame.payload {
            EthernetPayload::Ipv4(ipv4) => match ipv4.payload {
                Ipv4Payload::Tcp { packet, repr } => {
                    assert_eq!(repr.src_port, 1234);
                    assert_eq!(repr.dst_port, 80);
                    assert_eq!(repr.seq_num, 10);
                    assert_eq!(repr.ack_num, 20);
                    assert!(repr.flags.ack());
                    assert!(repr.flags.psh());
                    assert_eq!(packet.payload(), Ok(&payload[..]));
                }
                _ => panic!("expected TCP payload"),
            },
            _ => panic!("expected IPv4 frame"),
        }
    }
}
