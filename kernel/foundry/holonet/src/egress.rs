use crate::{
    NetworkError, NetworkResult,
    l2::ethernet::{self, EtherType, MacAddress},
    l3::ip::{self, v4 as ipv4},
    utils::ensure_len,
};

/// Default hop limit for IPv4 packets emitted by the stack.
pub const DEFAULT_IPV4_TTL: u8 = 64;
pub const ETHERNET_IPV4_HEADER_LEN: usize = ethernet::HEADER_LEN + ipv4::HEADER_LEN;

/// Ethernet II + IPv4 envelope used to emit protocol payloads.
///
/// The destination hardware address is the resolved next-hop address. For
/// off-link traffic this is the gateway MAC, not the final IPv4 destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthernetIpv4Envelope {
    pub source_hardware_addr: MacAddress,
    pub destination_hardware_addr: MacAddress,
    pub source_addr: ipv4::Ipv4Addr,
    pub destination_addr: ipv4::Ipv4Addr,
    pub protocol: ip::Protocol,
    pub payload_len: usize,
    pub ttl: u8,
    pub flags: ipv4::Flags,
}

impl EthernetIpv4Envelope {
    /// Return the number of bytes required for the complete Ethernet frame.
    #[inline]
    pub fn frame_len(&self) -> NetworkResult<usize> {
        self.payload_len
            .checked_add(ETHERNET_IPV4_HEADER_LEN)
            .ok_or(NetworkError::Oversized)
    }

    fn ipv4_packet_len(&self) -> NetworkResult<usize> {
        let packet_len = self
            .payload_len
            .checked_add(ipv4::HEADER_LEN)
            .ok_or(NetworkError::Oversized)?;

        if packet_len > usize::from(u16::MAX) {
            return Err(NetworkError::Oversized);
        }

        Ok(packet_len)
    }

    /// Emit the envelope and let the caller fill the IPv4 payload.
    ///
    /// The callback receives exactly `payload_len` bytes and must initialize
    /// the full slice before returning.
    pub fn emit_with<F>(&self, buffer: &mut [u8], emit_payload: F) -> NetworkResult<usize>
    where
        F: FnOnce(&mut [u8]) -> NetworkResult<()>,
    {
        let _ = self.ipv4_packet_len()?;
        let frame_len = self.frame_len()?;
        ensure_len(buffer, frame_len)?;

        let frame = &mut buffer[..frame_len];
        let mut ethernet = ethernet::Frame::new(frame)?;
        ethernet::Repr {
            src_addr: self.source_hardware_addr,
            dst_addr: self.destination_hardware_addr,
            ethertype: EtherType::IpV4,
        }
        .emit(&mut ethernet)?;

        let mut ipv4 = ipv4::Packet::new_unchecked(ethernet.payload_mut()?);
        ipv4::Repr {
            src_addr: self.source_addr,
            dst_addr: self.destination_addr,
            protocol: self.protocol,
            payload_len: self.payload_len,
            ttl: self.ttl,
            flags: self.flags,
        }
        .emit(&mut ipv4)?;

        emit_payload(ipv4.payload_mut()?)?;

        Ok(frame_len)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        NetworkError,
        ingress::{EthernetPayload, IngressFrame, Ipv4Payload},
    };

    const LOCAL_MAC: MacAddress = MacAddress::new([0x10, 0x11, 0x12, 0x13, 0x14, 0x15]);
    const PEER_MAC: MacAddress = MacAddress::new([0x20, 0x21, 0x22, 0x23, 0x24, 0x25]);
    const LOCAL_IP: ipv4::Ipv4Addr = ipv4::Ipv4Addr::new(192, 168, 1, 10);
    const PEER_IP: ipv4::Ipv4Addr = ipv4::Ipv4Addr::new(192, 168, 1, 1);

    fn envelope(payload_len: usize) -> EthernetIpv4Envelope {
        EthernetIpv4Envelope {
            source_hardware_addr: LOCAL_MAC,
            destination_hardware_addr: PEER_MAC,
            source_addr: LOCAL_IP,
            destination_addr: PEER_IP,
            protocol: ip::Protocol::Igmp,
            payload_len,
            ttl: DEFAULT_IPV4_TTL,
            flags: ipv4::Flags::new(true, false),
        }
    }

    #[test]
    fn test_emit_ipv4_envelope() {
        let payload = [0xDE, 0xAD, 0xBE, 0xEF];
        let envelope = envelope(payload.len());
        let mut bytes = [0u8; ethernet::HEADER_LEN + ipv4::HEADER_LEN + 4];

        let len = envelope
            .emit_with(&mut bytes, |payload_buffer| {
                payload_buffer.copy_from_slice(&payload);
                Ok(())
            })
            .unwrap();

        assert_eq!(len, bytes.len());

        let frame = IngressFrame::parse(&bytes).unwrap();
        assert_eq!(frame.frame.src_addr(), Ok(LOCAL_MAC));
        assert_eq!(frame.frame.dst_addr(), Ok(PEER_MAC));

        match frame.payload {
            EthernetPayload::Ipv4(ipv4) => {
                assert_eq!(ipv4.packet.src_addr(), Ok(LOCAL_IP));
                assert_eq!(ipv4.packet.dst_addr(), Ok(PEER_IP));
                assert_eq!(ipv4.packet.ttl(), Ok(DEFAULT_IPV4_TTL));

                match ipv4.payload {
                    Ipv4Payload::Unsupported {
                        protocol,
                        payload: parsed_payload,
                    } => {
                        assert_eq!(protocol, ip::Protocol::Igmp as u8);
                        assert_eq!(parsed_payload, payload);
                    }
                    _ => panic!("expected unsupported payload"),
                }
            }
            _ => panic!("expected IPv4 frame"),
        }
    }

    #[test]
    fn test_emit_ipv4_envelope_rejects_truncated_buffer() {
        let envelope = envelope(4);
        let mut bytes = [0u8; ethernet::HEADER_LEN + ipv4::HEADER_LEN + 3];

        let err = envelope.emit_with(&mut bytes, |_| Ok(())).unwrap_err();

        assert_eq!(err, NetworkError::Truncated);
    }
}
