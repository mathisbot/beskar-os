pub use core::net::{SocketAddr, SocketAddrV4, SocketAddrV6};

use crate::{
    NetworkError, NetworkResult,
    egress::EthernetIpv4Envelope,
    l3::ip::v4::Ipv4Addr,
    utils::{
        checksum_with_pseudo, ensure_len, read_u16, slice, slice_mut, usize_to_u16, write_u16,
    },
};

/// Range of bytes for the source port field.
const SOURCE_PORT: usize = 0;
/// Range of bytes for the destination port field.
const DEST_PORT: usize = 2;
/// Range of bytes for the length field.
const LENGTH: usize = 4;
/// Range of bytes for the checksum field.
const CHECKSUM: usize = 6;
/// Length of the UDP header (fixed).
pub const HEADER_LEN: usize = 8;

/// Borrowed UDP datagram metadata and payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Datagram<'a> {
    pub source_addr: Ipv4Addr,
    pub destination_addr: Ipv4Addr,
    pub source_port: u16,
    pub destination_port: u16,
    pub payload: &'a [u8],
}

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
    /// Returns `Invalid` if the buffer is too short or if the length field is inconsistent.
    pub fn check_len(&self) -> NetworkResult<()> {
        let data = self.buffer.as_ref();
        ensure_len(data, HEADER_LEN)?;

        let len = usize::from(read_u16(data, LENGTH)?);
        if len < HEADER_LEN || len > data.len() {
            return Err(NetworkError::Invalid);
        }

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
    /// Return the length field.
    pub fn len(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), LENGTH)
    }

    #[must_use]
    #[inline]
    /// Return the checksum field.
    pub fn checksum(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), CHECKSUM)
    }

    #[must_use]
    #[inline]
    /// Return whether the packet has a valid checksum (non-zero for UDP/IPv4).
    pub fn has_checksum(&self) -> NetworkResult<bool> {
        Ok(self.checksum()? != 0)
    }

    #[must_use]
    #[inline]
    /// Return the payload.
    pub fn payload(&self) -> NetworkResult<&[u8]> {
        let data = self.buffer.as_ref();
        let len = usize::from(self.len()?);
        slice(data, HEADER_LEN..len)
    }

    #[must_use]
    #[inline]
    /// Return whether this packet is empty (only contains the header).
    pub fn is_empty(&self) -> NetworkResult<bool> {
        Ok(usize::from(self.len()?) <= HEADER_LEN)
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
    /// Set the length field.
    pub fn set_len(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), LENGTH, value)
    }

    #[inline]
    /// Set the checksum field.
    pub fn set_checksum(&mut self, value: u16) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), CHECKSUM, value)
    }

    #[inline]
    /// Get a mutable reference to the payload.
    pub fn payload_mut(&mut self) -> NetworkResult<&mut [u8]> {
        let len = usize::from(self.len()?);
        ensure_len(self.buffer.as_ref(), len)?;
        slice_mut(self.buffer.as_mut(), HEADER_LEN..len)
    }

    /// Recalculate and set the UDP checksum with pseudo-header (IPv4).
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the packet length field is inconsistent.
    /// Returns `Oversized` if the UDP length does not fit into the pseudo-header.
    pub fn fill_checksum(&mut self, src_addr: Ipv4Addr, dst_addr: Ipv4Addr) -> NetworkResult<()> {
        let len = usize::from(self.len()?);
        if len < HEADER_LEN {
            return Err(NetworkError::Invalid);
        }
        ensure_len(self.buffer.as_ref(), len)?;

        // Build pseudo-header
        let mut pseudo = [0u8; 12];
        pseudo[0..4].copy_from_slice(&src_addr.octets());
        pseudo[4..8].copy_from_slice(&dst_addr.octets());
        pseudo[8] = 0; // Reserved
        pseudo[9] = 17; // Protocol (UDP)
        pseudo[10..12].copy_from_slice(&usize_to_u16(len)?.to_be_bytes());

        self.set_checksum(0)?;
        let data = self.buffer.as_ref();
        let cksum = checksum_with_pseudo(&pseudo, &data[..len]);

        // UDP checksum should never be 0 in IPv4
        self.set_checksum(if cksum == 0 { 0xFFFF } else { cksum })
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
            src_port: packet.src_port()?,
            dst_port: packet.dst_port()?,
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

    /// Emit a high-level representation into a UDP packet.
    ///
    /// # Errors
    ///
    /// Returns `Truncated` if the packet buffer is too short.
    /// Returns `Oversized` if the payload length does not fit into the UDP length field.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) -> NetworkResult<()> {
        let packet_len = self.packet_len()?;
        ensure_len(packet.buffer.as_ref(), packet_len)?;
        packet.set_src_port(self.src_port)?;
        packet.set_dst_port(self.dst_port)?;
        packet.set_len(usize_to_u16(packet_len)?)
    }
}

/// Emit a UDP datagram inside an Ethernet + IPv4 frame.
///
/// `destination_hardware_addr` is the resolved next-hop Ethernet address.
#[allow(clippy::too_many_arguments)]
pub fn emit_ipv4(
    mut envelope: EthernetIpv4Envelope,
    buffer: &mut [u8],
    source_port: u16,
    destination_port: u16,
    payload: &[u8],
) -> NetworkResult<usize> {
    let udp_len = payload
        .len()
        .checked_add(HEADER_LEN)
        .ok_or(NetworkError::Oversized)?;
    envelope.payload_len = udp_len;
    envelope.emit_with(buffer, |payload_buffer| {
        let mut packet = Packet::new_unchecked(payload_buffer);
        Repr {
            src_port: source_port,
            dst_port: destination_port,
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
        assert_eq!(packet.src_port(), Ok(53));
        assert_eq!(packet.dst_port(), Ok(4660));
    }

    #[test]
    fn test_len() {
        let packet = Packet::new_unchecked(&PACKET_BYTES[..]);
        assert_eq!(packet.len(), Ok(8));
    }

    #[test]
    fn test_construct() {
        let mut bytes = vec![0u8; 8];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_src_port(53).unwrap();
        packet.set_dst_port(4660).unwrap();
        packet.set_len(8).unwrap();
        packet.set_checksum(0).unwrap();

        assert_eq!(packet.src_port(), Ok(53));
        assert_eq!(packet.dst_port(), Ok(4660));
        assert_eq!(packet.len(), Ok(8));
    }

    #[test]
    fn test_check_len_rejects_invalid_length_field() {
        let mut bytes = PACKET_BYTES;
        bytes[4] = 0x00;
        bytes[5] = 0x07;
        let packet = Packet::new_unchecked(bytes);
        assert_eq!(packet.check_len(), Err(NetworkError::Invalid));
    }

    #[test]
    fn test_check_len_rejects_length_larger_than_buffer() {
        let mut bytes = PACKET_BYTES;
        bytes[4] = 0x00;
        bytes[5] = 0x20;
        let packet = Packet::new_unchecked(bytes);
        assert_eq!(packet.check_len(), Err(NetworkError::Invalid));
    }
}
