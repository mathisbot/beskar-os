use crate::{
    NetworkResult,
    l2::ethernet::{self, EtherType},
    l3::{
        arp,
        ip::{self, v4 as ipv4},
    },
    l4::{icmp, tcp, udp},
};

/// A parsed Ethernet ingress frame.
#[derive(Debug, Clone)]
pub struct IngressFrame<'a> {
    pub frame: ethernet::Frame<&'a [u8]>,
    pub payload: EthernetPayload<'a>,
}

/// A decoded Ethernet payload.
#[derive(Debug, Clone)]
pub enum EthernetPayload<'a> {
    Arp {
        packet: arp::Packet<&'a [u8]>,
        repr: arp::Repr,
    },
    Ipv4(Ipv4Packet<'a>),
    Ipv6(&'a [u8]),
    Unsupported {
        ethertype: u16,
        payload: &'a [u8],
    },
}

/// A decoded IPv4 packet and its transport payload classification.
#[derive(Debug, Clone)]
pub struct Ipv4Packet<'a> {
    pub packet: ipv4::Packet<&'a [u8]>,
    pub payload: Ipv4Payload<'a>,
}

/// A decoded IPv4 transport payload.
#[derive(Debug, Clone)]
pub enum Ipv4Payload<'a> {
    Icmp {
        packet: icmp::Packet<&'a [u8]>,
        repr: icmp::Repr,
    },
    Tcp {
        packet: tcp::Packet<&'a [u8]>,
        repr: tcp::Repr,
    },
    Udp {
        packet: udp::Packet<&'a [u8]>,
        repr: udp::Repr,
    },
    Unsupported {
        protocol: u8,
        payload: &'a [u8],
    },
}

impl<'a> IngressFrame<'a> {
    /// Parse and classify a received Ethernet frame.
    ///
    /// # Errors
    ///
    /// Returns protocol-specific parsing errors for malformed frames.
    pub fn parse(bytes: &'a [u8]) -> NetworkResult<Self> {
        let frame = ethernet::Frame::new(bytes)?;

        let ethertype_raw = frame.ethertype_raw()?;

        let payload_bytes = &bytes[ethernet::HEADER_LEN..];
        let payload = match frame.ethertype() {
            Ok(EtherType::Arp) => {
                let packet = arp::Packet::new(payload_bytes)?;
                let repr = arp::Repr::parse(&packet)?;
                EthernetPayload::Arp { packet, repr }
            }
            Ok(EtherType::IpV4) => EthernetPayload::Ipv4(Ipv4Packet::parse(payload_bytes)?),
            Ok(EtherType::IpV6) => EthernetPayload::Ipv6(payload_bytes),
            Err(_) => EthernetPayload::Unsupported {
                ethertype: ethertype_raw,
                payload: payload_bytes,
            },
        };

        Ok(Self { frame, payload })
    }
}

impl<'a> Ipv4Packet<'a> {
    /// Parse and classify an IPv4 packet payload.
    ///
    /// # Errors
    ///
    /// Returns protocol-specific parsing errors for malformed packets.
    pub fn parse(bytes: &'a [u8]) -> NetworkResult<Self> {
        let packet = ipv4::Packet::new(bytes)?;

        let protocol_raw = packet.protocol_raw()?;

        let payload_start = packet.header_len()?;
        let payload_end = usize::from(packet.total_len()?);
        let payload_bytes = &bytes[payload_start..payload_end];
        let payload = match packet.protocol() {
            Ok(ip::Protocol::Icmp) => {
                let packet = icmp::Packet::new(payload_bytes)?;
                let repr = icmp::Repr::parse(&packet)?;
                Ipv4Payload::Icmp { packet, repr }
            }
            Ok(ip::Protocol::Tcp) => {
                let packet = tcp::Packet::new(payload_bytes)?;
                let repr = tcp::Repr::parse(&packet)?;
                Ipv4Payload::Tcp { packet, repr }
            }
            Ok(ip::Protocol::Udp) => {
                let packet = udp::Packet::new(payload_bytes)?;
                let repr = udp::Repr::parse(&packet)?;
                Ipv4Payload::Udp { packet, repr }
            }
            Ok(ip::Protocol::Igmp) | Err(_) => Ipv4Payload::Unsupported {
                protocol: protocol_raw,
                payload: payload_bytes,
            },
        };

        Ok(Self { packet, payload })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::l3::arp::Operation;

    const ETH_ARP_FRAME: [u8; 42] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x08, 0x06, 0x00,
        0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x21, 0x22,
        0x23, 0x24, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x41, 0x42, 0x43, 0x44,
    ];

    const ETH_IPV4_UDP_FRAME: [u8; 42] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x08, 0x00, 0x45,
        0x00, 0x00, 0x1C, 0x12, 0x34, 0x40, 0x00, 0x40, 0x11, 0x00, 0x00, 0xC0, 0xA8, 0x00, 0x01,
        0xC0, 0xA8, 0x00, 0x02, 0x00, 0x35, 0x12, 0x34, 0x00, 0x08, 0x00, 0x00,
    ];

    const ETH_IPV4_UNKNOWN_FRAME: [u8; 42] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x08, 0x00, 0x45,
        0x00, 0x00, 0x1C, 0x12, 0x34, 0x40, 0x00, 0x40, 0xFA, 0x00, 0x00, 0xC0, 0xA8, 0x00, 0x01,
        0xC0, 0xA8, 0x00, 0x02, 0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE,
    ];

    #[test]
    fn test_parse_arp_ingress_frame() {
        let frame = IngressFrame::parse(&ETH_ARP_FRAME).unwrap();

        match frame.payload {
            EthernetPayload::Arp { repr, .. } => match repr {
                arp::Repr::EthernetIpv4 { operation, .. } => {
                    assert_eq!(operation, Operation::Request);
                }
            },
            _ => panic!("expected ARP frame"),
        }
    }

    #[test]
    fn test_parse_ipv4_udp_ingress_frame() {
        let frame = IngressFrame::parse(&ETH_IPV4_UDP_FRAME).unwrap();

        match frame.payload {
            EthernetPayload::Ipv4(ipv4) => match ipv4.payload {
                Ipv4Payload::Udp { repr, .. } => {
                    assert_eq!(repr.src_port, 53);
                    assert_eq!(repr.dst_port, 0x1234);
                    assert_eq!(repr.payload_len, 0);
                }
                _ => panic!("expected UDP payload"),
            },
            _ => panic!("expected IPv4 frame"),
        }
    }

    #[test]
    fn test_parse_unknown_ipv4_payload() {
        let frame = IngressFrame::parse(&ETH_IPV4_UNKNOWN_FRAME).unwrap();

        match frame.payload {
            EthernetPayload::Ipv4(ipv4) => match ipv4.payload {
                Ipv4Payload::Unsupported { protocol, payload } => {
                    assert_eq!(protocol, 0xFA);
                    assert_eq!(payload, &[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE]);
                }
                _ => panic!("expected unsupported payload"),
            },
            _ => panic!("expected IPv4 frame"),
        }
    }

    #[test]
    fn test_transport_views_preserve_packet_access() {
        let frame = IngressFrame::parse(&ETH_IPV4_UDP_FRAME).unwrap();

        match frame.payload {
            EthernetPayload::Ipv4(ipv4) => match ipv4.payload {
                Ipv4Payload::Udp { packet, .. } => {
                    assert_eq!(packet.src_port(), Ok(53));
                    assert_eq!(packet.dst_port(), Ok(0x1234));
                }
                Ipv4Payload::Tcp { packet, repr } => {
                    assert_eq!(packet.window_size(), Ok(repr.window_size));
                }
                Ipv4Payload::Icmp { .. } | Ipv4Payload::Unsupported { .. } => {
                    panic!("expected UDP payload")
                }
            },
            _ => panic!("expected IPv4 frame"),
        }
    }
}
