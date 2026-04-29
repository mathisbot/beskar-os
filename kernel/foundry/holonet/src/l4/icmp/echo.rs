use crate::{
    NetworkError, NetworkResult,
    egress::{self, EthernetIpv4Envelope},
    ingress::{EthernetPayload, IngressFrame, Ipv4Payload},
    l3::ip::v4 as ipv4,
};

use super::{HEADER_LEN, MessageType, Packet};

/// Metadata for an ICMP echo reply addressed to a local IPv4 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EchoReply {
    pub source_addr: ipv4::Ipv4Addr,
    pub identifier: u16,
    pub sequence: u16,
    pub payload_len: usize,
}

/// Return the static header length for an Ethernet + IPv4 + ICMP echo request.
#[must_use]
#[inline]
pub const fn request_header_len() -> usize {
    egress::ETHERNET_IPV4_HEADER_LEN + HEADER_LEN
}

/// Return a complete Ethernet + IPv4 + ICMP echo request frame length.
#[inline]
pub fn request_frame_len(payload_len: usize) -> NetworkResult<usize> {
    let icmp_len = payload_len
        .checked_add(HEADER_LEN)
        .ok_or(NetworkError::Oversized)?;
    let ipv4_len = icmp_len
        .checked_add(ipv4::HEADER_LEN)
        .ok_or(NetworkError::Oversized)?;

    if ipv4_len > usize::from(u16::MAX) {
        return Err(NetworkError::Oversized);
    }

    payload_len
        .checked_add(request_header_len())
        .ok_or(NetworkError::Oversized)
}

/// Emit an ICMP echo request inside an Ethernet + IPv4 frame.
///
/// `destination_hardware_addr` is the resolved next-hop Ethernet address.
#[allow(clippy::too_many_arguments)]
pub fn emit_request(
    mut envelope: EthernetIpv4Envelope,
    buffer: &mut [u8],
    identifier: u16,
    sequence: u16,
    payload: &[u8],
) -> NetworkResult<usize> {
    let payload_len = payload
        .len()
        .checked_add(HEADER_LEN)
        .ok_or(NetworkError::Oversized)?;
    envelope.payload_len = payload_len;
    envelope.emit_with(buffer, |payload_buffer| {
        let mut packet = Packet::new(payload_buffer)?;
        packet.set_msg_type(MessageType::EchoRequest)?;
        packet.set_code(0)?;
        packet.set_echo_identity(identifier, sequence)?;
        packet.payload_mut()?.copy_from_slice(payload);
        packet.fill_checksum()
    })
}

/// Recognize an ICMP echo reply addressed to `local_addr`.
pub fn parse_reply(
    local_addr: ipv4::Ipv4Addr,
    frame: &IngressFrame<'_>,
) -> NetworkResult<Option<EchoReply>> {
    let EthernetPayload::Ipv4(ipv4) = &frame.payload else {
        return Ok(None);
    };

    if ipv4.packet.dst_addr()? != local_addr {
        return Ok(None);
    }

    let Ipv4Payload::Icmp { packet, repr } = &ipv4.payload else {
        return Ok(None);
    };

    if repr.msg_type != MessageType::EchoReply || repr.code != 0 {
        return Ok(None);
    }

    let (identifier, sequence) = packet.echo_identity()?;
    Ok(Some(EchoReply {
        source_addr: ipv4.packet.src_addr()?,
        identifier,
        sequence,
        payload_len: repr.payload_len,
    }))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        ingress::IngressFrame,
        l2::ethernet::{self, EtherType, MacAddress},
        l3::ip,
    };

    const LOCAL_MAC: MacAddress = MacAddress::new([0x10, 0x11, 0x12, 0x13, 0x14, 0x15]);
    const PEER_MAC: MacAddress = MacAddress::new([0x20, 0x21, 0x22, 0x23, 0x24, 0x25]);
    const LOCAL_IP: ipv4::Ipv4Addr = ipv4::Ipv4Addr::new(192, 168, 1, 10);
    const REMOTE_IP: ipv4::Ipv4Addr = ipv4::Ipv4Addr::new(8, 8, 8, 8);

    #[test]
    fn test_emit_icmp_echo_request_frame() {
        let payload = [0xCA, 0xFE, 0xBA, 0xBE];
        let mut bytes = [0u8; request_header_len() + 4];

        let envelope = EthernetIpv4Envelope {
            source_hardware_addr: LOCAL_MAC,
            destination_hardware_addr: PEER_MAC,
            source_addr: LOCAL_IP,
            destination_addr: REMOTE_IP,
            protocol: ip::Protocol::Icmp,
            payload_len: 0, // will be set by emit_request
            ttl: 64,
            flags: ipv4::Flags::new(true, false),
        };

        let len = emit_request(envelope, &mut bytes, 0x1234, 7, &payload).unwrap();
        assert_eq!(len, bytes.len());

        let frame = IngressFrame::parse(&bytes).unwrap();
        assert_eq!(frame.frame.src_addr(), Ok(LOCAL_MAC));
        assert_eq!(frame.frame.dst_addr(), Ok(PEER_MAC));

        match frame.payload {
            EthernetPayload::Ipv4(ipv4) => {
                assert_eq!(ipv4.packet.src_addr(), Ok(LOCAL_IP));
                assert_eq!(ipv4.packet.dst_addr(), Ok(REMOTE_IP));
                match ipv4.payload {
                    Ipv4Payload::Icmp { packet, repr } => {
                        assert_eq!(repr.msg_type, MessageType::EchoRequest);
                        assert_eq!(packet.echo_identity(), Ok((0x1234, 7)));
                        assert_eq!(packet.payload(), Ok(&payload[..]));
                    }
                    _ => panic!("expected ICMP payload"),
                }
            }
            _ => panic!("expected IPv4 frame"),
        }
    }

    #[test]
    fn test_parse_icmp_echo_reply() {
        let payload = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut bytes = [0u8; ethernet::HEADER_LEN + ipv4::HEADER_LEN + HEADER_LEN + 8];

        let mut frame = ethernet::Frame::new(&mut bytes[..]).unwrap();
        ethernet::Repr {
            src_addr: PEER_MAC,
            dst_addr: LOCAL_MAC,
            ethertype: EtherType::IpV4,
        }
        .emit(&mut frame)
        .unwrap();

        let mut ipv4 = ipv4::Packet::new_unchecked(frame.payload_mut().unwrap());
        ipv4::Repr {
            src_addr: REMOTE_IP,
            dst_addr: LOCAL_IP,
            protocol: ip::Protocol::Icmp,
            payload_len: HEADER_LEN + payload.len(),
            ttl: 57,
            flags: ipv4::Flags::new(false, false),
        }
        .emit(&mut ipv4)
        .unwrap();

        let mut icmp = Packet::new(ipv4.payload_mut().unwrap()).unwrap();
        icmp.set_msg_type(MessageType::EchoReply).unwrap();
        icmp.set_code(0).unwrap();
        icmp.set_echo_identity(0xBEEF, 42).unwrap();
        icmp.payload_mut().unwrap().copy_from_slice(&payload);
        icmp.fill_checksum().unwrap();

        let frame = IngressFrame::parse(&bytes).unwrap();
        assert_eq!(
            parse_reply(LOCAL_IP, &frame).unwrap(),
            Some(EchoReply {
                source_addr: REMOTE_IP,
                identifier: 0xBEEF,
                sequence: 42,
                payload_len: payload.len(),
            })
        );
    }
}
