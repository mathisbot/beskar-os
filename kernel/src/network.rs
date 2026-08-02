//! IPv4 network interface.
//!
//! `holonet` knows how to parse and emit frames but performs no I/O, and the
//! NIC drivers move frames but understand no protocol. This module is the layer
//! in between: it owns the interface's routing state and neighbor cache, drives
//! the receive path, and turns them into operations such as [`ping`].
//!
//! The interface is deliberately minimal. There is no socket layer and no DHCP
//! client yet, so the addressing is static (see [`default_route`]) and the only
//! transport driven from here is ICMP echo.

use crate::drivers::nic;
use crate::time::{Duration, Instant};
use holonet::{
    NetworkError, NetworkResult, Nic,
    egress::{DEFAULT_IPV4_TTL, EthernetIpv4Envelope},
    ingress::{EthernetPayload, IngressFrame, Ipv4Payload},
    l2::ethernet::{self, MacAddress},
    l3::{
        arp::{self, Operation, state as arp_state},
        ip::{self, v4 as ipv4},
        route::{Ipv4Cidr, Ipv4Route},
    },
    l4::icmp::{self, MessageType, echo},
};
use hyperdrive::locks::mcs::MUMcsLock;
use ipv4::Ipv4Addr;

/// Largest Ethernet frame the interface emits, for a standard 1500 byte MTU.
const FRAME_BUFFER_LEN: usize = ethernet::HEADER_LEN + 1500;

/// Number of neighbors kept in the ARP cache.
const ARP_CACHE_CAPACITY: usize = 16;
/// Number of hash buckets backing the ARP cache.
const ARP_CACHE_BUCKETS: usize = 8;
/// Lifetime of an ARP cache entry, in milliseconds.
///
/// The unit is milliseconds because that is the tick the cache clock is
/// advanced in by `refresh_arp_clock`.
const ARP_CACHE_TTL_MS: u64 = 60 * 1_000;

/// How long a single ARP request waits for its reply.
const ARP_REPLY_TIMEOUT: Duration = Duration::from_millis(500);
/// Number of ARP requests sent before a neighbor is declared unreachable.
const ARP_ATTEMPTS: u8 = 3;

/// Default time an echo request waits for its reply.
pub const DEFAULT_ECHO_TIMEOUT: Duration = Duration::from_secs(1);
/// Number of payload bytes carried by an echo request.
const ECHO_PAYLOAD_LEN: usize = 32;

static INTERFACE: MUMcsLock<Interface> = MUMcsLock::uninit();

/// The addressing used by the QEMU user-networking setup the build tool starts
/// (`user,net=192.168.1.0/24,host=192.168.1.1`).
///
/// The address is the one the SLIRP DHCP server would hand out, so the
/// interface is usable without a DHCP client.
///
/// # Panics
///
/// Never: the prefix length is in range and the gateway is on-link.
#[must_use]
pub fn default_route() -> Ipv4Route {
    let cidr = Ipv4Cidr::new(Ipv4Addr::new(192, 168, 1, 15), 24).unwrap();
    Ipv4Route::new(cidr, Some(Ipv4Addr::new(192, 168, 1, 1))).unwrap()
}

/// Outcome of a successful echo exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EchoOutcome {
    /// Address that answered, which may differ from the one requested.
    pub source_addr: Ipv4Addr,
    /// Time between emitting the request and receiving the reply.
    pub round_trip: Duration,
    /// Sequence number the reply carried.
    pub sequence: u16,
}

/// Anything the receive path decoded that a caller may be waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Received {
    /// A neighbor advertised its hardware address.
    Neighbor { addr: Ipv4Addr },
    /// An echo reply addressed to this host.
    Echo {
        source_addr: Ipv4Addr,
        identifier: u16,
        sequence: u16,
    },
}

/// A response the receive path has to emit once it is done reading the frame
/// that triggered it.
///
/// The parsed frame borrows the driver's receive buffer, so nothing can be
/// transmitted while it is alive; the response is staged here instead.
#[derive(Debug, Clone, Copy)]
enum Pending {
    None,
    ArpReply {
        target_hardware_addr: MacAddress,
        target_protocol_addr: Ipv4Addr,
    },
    EchoReply {
        destination_hardware_addr: MacAddress,
        destination_addr: Ipv4Addr,
        identifier: u16,
        sequence: u16,
        payload_len: usize,
    },
}

/// The system IPv4 interface.
pub struct Interface {
    mac: MacAddress,
    route: Ipv4Route,
    arp: arp_state::Cache<ARP_CACHE_CAPACITY, ARP_CACHE_BUCKETS>,
    /// Last instant the ARP cache's logical clock was advanced.
    arp_clock: Instant,
    /// Identifier stamped on the echo requests emitted by this interface.
    identifier: u16,
    /// Sequence number of the next echo request.
    sequence: u16,
    tx: [u8; FRAME_BUFFER_LEN],
}

impl Interface {
    fn new(mac: MacAddress, route: Ipv4Route) -> NetworkResult<Self> {
        // The cache clock is driven in milliseconds by `refresh_arp_clock`.
        let arp = arp_state::Cache::new(ARP_CACHE_TTL_MS)?;
        let mac_bytes = mac.as_bytes();

        Ok(Self {
            mac,
            route,
            arp,
            arp_clock: Instant::now(),
            // The identifier only has to tell this interface's requests apart
            // from those of other hosts sharing the reply path.
            identifier: u16::from_be_bytes([mac_bytes[4], mac_bytes[5]]),
            sequence: 0,
            tx: [0; FRAME_BUFFER_LEN],
        })
    }

    #[must_use]
    #[inline]
    pub const fn mac_address(&self) -> MacAddress {
        self.mac
    }

    #[must_use]
    #[inline]
    pub const fn route(&self) -> Ipv4Route {
        self.route
    }

    /// Replace the addressing of the interface.
    ///
    /// The neighbor cache is dropped, as its entries were learned under the
    /// previous prefix.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the ARP cache cannot be rebuilt.
    pub fn set_route(&mut self, route: Ipv4Route) -> NetworkResult<()> {
        self.route = route;
        self.arp = arp_state::Cache::new(ARP_CACHE_TTL_MS)?;
        self.arp_clock = Instant::now();

        Ok(())
    }

    /// Advance the ARP cache's logical clock to match the wall clock.
    ///
    /// The cache expires entries in ticks supplied by its owner; this converts
    /// elapsed real time into those ticks and purges whatever went stale.
    fn refresh_arp_clock(&mut self) {
        let now = Instant::now();
        let elapsed = u64::try_from((now - self.arp_clock).as_millis()).unwrap_or(u64::MAX);

        // Below a millisecond there is nothing to advance, and moving the
        // instant anyway would round the remainder away on every poll.
        if elapsed > 0 {
            self.arp.advance(elapsed);
            self.arp_clock = now;
        }
    }

    /// Decode one received frame, reporting what it carried and what has to be
    /// sent back.
    fn classify(
        &self,
        bytes: &[u8],
        echo_payload: &mut [u8; ECHO_PAYLOAD_LEN],
    ) -> (Option<(Ipv4Addr, MacAddress)>, Option<Received>, Pending) {
        let Ok(frame) = IngressFrame::parse(bytes) else {
            return (None, None, Pending::None);
        };

        match &frame.payload {
            EthernetPayload::Arp(packet) => {
                let Ok(arp::Repr::EthernetIpv4 {
                    operation,
                    source_hardware_addr,
                    source_protocol_addr,
                    target_protocol_addr,
                    ..
                }) = arp::Repr::parse(packet)
                else {
                    return (None, None, Pending::None);
                };

                // Requests and replies both carry a usable mapping for their
                // sender, so the cache is filled either way.
                let learned = Some((source_protocol_addr, source_hardware_addr));
                let received = Some(Received::Neighbor {
                    addr: source_protocol_addr,
                });

                let pending = if operation == Operation::Request
                    && target_protocol_addr == self.route.addr()
                {
                    Pending::ArpReply {
                        target_hardware_addr: source_hardware_addr,
                        target_protocol_addr: source_protocol_addr,
                    }
                } else {
                    Pending::None
                };

                (learned, received, pending)
            }
            EthernetPayload::Ipv4(packet) => {
                let Ipv4Payload::Icmp(icmp_packet) = &packet.payload else {
                    return (None, None, Pending::None);
                };

                if packet.packet.dst_addr() != Ok(self.route.addr()) {
                    return (None, None, Pending::None);
                }

                let (Ok(repr), Ok((identifier, sequence)), Ok(source_addr)) = (
                    icmp::Repr::parse(icmp_packet),
                    icmp_packet.echo_identity(),
                    packet.packet.src_addr(),
                ) else {
                    return (None, None, Pending::None);
                };

                if repr.code != 0 {
                    return (None, None, Pending::None);
                }

                match repr.msg_type {
                    MessageType::EchoReply => (
                        None,
                        Some(Received::Echo {
                            source_addr,
                            identifier,
                            sequence,
                        }),
                        Pending::None,
                    ),
                    MessageType::EchoRequest => {
                        let (Ok(payload), Ok(destination_hardware_addr)) =
                            (icmp_packet.payload(), frame.frame.src_addr())
                        else {
                            return (None, None, Pending::None);
                        };

                        // Echo back at most what the staging buffer holds; a
                        // longer request is answered short rather than dropped.
                        let payload_len = payload.len().min(ECHO_PAYLOAD_LEN);
                        echo_payload[..payload_len].copy_from_slice(&payload[..payload_len]);

                        (
                            None,
                            None,
                            Pending::EchoReply {
                                destination_hardware_addr,
                                destination_addr: source_addr,
                                identifier,
                                sequence,
                                payload_len,
                            },
                        )
                    }
                    _ => (None, None, Pending::None),
                }
            }
            EthernetPayload::Ipv6(_) | EthernetPayload::Unsupported { .. } => {
                (None, None, Pending::None)
            }
        }
    }

    /// Emit whatever the receive path staged.
    fn answer(&mut self, nic: &mut dyn Nic, pending: Pending, echo_payload: &[u8]) {
        match pending {
            Pending::None => {}
            Pending::ArpReply {
                target_hardware_addr,
                target_protocol_addr,
            } => {
                let reply = arp_state::EthernetIpv4Frame::reply(
                    self.mac,
                    self.route.addr(),
                    target_hardware_addr,
                    target_protocol_addr,
                );
                let len = reply.buffer_len();

                if reply.emit(&mut self.tx[..len]).is_ok() {
                    let _ = nic.send_frame(&self.tx[..len]);
                }
            }
            Pending::EchoReply {
                destination_hardware_addr,
                destination_addr,
                identifier,
                sequence,
                payload_len,
            } => {
                let _ = emit_echo(
                    &mut self.tx,
                    nic,
                    MessageType::EchoReply,
                    EchoTarget {
                        local_mac: self.mac,
                        peer_mac: destination_hardware_addr,
                        local_ip: self.route.addr(),
                        peer_ip: destination_addr,
                    },
                    identifier,
                    sequence,
                    &echo_payload[..payload_len],
                );
            }
        }
    }

    /// Process a single received frame, answering what can be answered inline,
    /// and report anything a caller might be waiting for.
    ///
    /// Returns `None` when no frame was pending.
    fn service_once(&mut self, nic: &mut dyn Nic) -> Option<Received> {
        // Staged out of the receive buffer, whose borrow ends at `consume_frame`.
        let mut echo_payload = [0u8; ECHO_PAYLOAD_LEN];

        let bytes = nic.poll_frame()?;
        let (learned, received, pending) = self.classify(bytes, &mut echo_payload);

        nic.consume_frame();

        if let Some((protocol_addr, hardware_addr)) = learned {
            self.refresh_arp_clock();
            let _ = self.arp.insert(protocol_addr, hardware_addr);
        }

        self.answer(nic, pending, &echo_payload);

        received
    }

    /// Drain the receive queue until `deadline`, stopping as soon as `wanted`
    /// accepts one of the decoded events.
    ///
    /// Frames that do not match keep being serviced, so ARP requests are still
    /// answered while waiting for something else.
    fn poll_until<F>(&mut self, nic: &mut dyn Nic, deadline: Instant, wanted: F) -> Option<Received>
    where
        F: Fn(&Received) -> bool,
    {
        loop {
            while let Some(received) = self.service_once(nic) {
                if wanted(&received) {
                    return Some(received);
                }
            }

            if Instant::now() >= deadline {
                return None;
            }

            core::hint::spin_loop();
        }
    }

    /// Resolve the hardware address of an on-link neighbor.
    ///
    /// Answers from the cache when possible, otherwise emits ARP requests until
    /// one is answered or the attempts run out.
    fn resolve(&mut self, nic: &mut dyn Nic, addr: Ipv4Addr) -> NetworkResult<MacAddress> {
        self.refresh_arp_clock();
        if let Some(mac) = self.arp.resolve(addr) {
            return Ok(mac);
        }

        for _ in 0..ARP_ATTEMPTS {
            let request = arp_state::EthernetIpv4Frame::request(self.mac, self.route.addr(), addr);
            let len = request.buffer_len();
            request.emit(&mut self.tx[..len])?;
            nic.send_frame(&self.tx[..len])?;

            let deadline = Instant::now() + ARP_REPLY_TIMEOUT;
            let _ = self.poll_until(
                nic,
                deadline,
                |received| matches!(received, Received::Neighbor { addr: seen } if *seen == addr),
            );

            // The mapping is taken from the cache rather than from the event: a
            // neighbor that answered an earlier request while we were waiting
            // is just as good.
            self.refresh_arp_clock();
            if let Some(mac) = self.arp.resolve(addr) {
                return Ok(mac);
            }
        }

        Err(NetworkError::Unreachable)
    }

    /// Send one ICMP echo request to `addr` and wait for its reply.
    fn echo(
        &mut self,
        nic: &mut dyn Nic,
        addr: Ipv4Addr,
        timeout: Duration,
    ) -> NetworkResult<EchoOutcome> {
        let next_hop = self.route.next_hop(addr)?;
        let destination_hardware_addr = self.resolve(nic, next_hop)?;

        let identifier = self.identifier;
        let sequence = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);

        // A recognizable, non-uniform pattern, so that a corrupted reply stands
        // out instead of matching an all-zero buffer.
        let mut payload = [0u8; ECHO_PAYLOAD_LEN];
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte = u8::try_from(index).unwrap_or(u8::MAX);
        }

        emit_echo(
            &mut self.tx,
            nic,
            MessageType::EchoRequest,
            EchoTarget {
                local_mac: self.mac,
                peer_mac: destination_hardware_addr,
                local_ip: self.route.addr(),
                peer_ip: addr,
            },
            identifier,
            sequence,
            &payload,
        )?;
        let sent_at = Instant::now();

        let received = self.poll_until(nic, sent_at + timeout, |received| {
            matches!(
                received,
                Received::Echo {
                    identifier: reply_identifier,
                    sequence: reply_sequence,
                    ..
                } if *reply_identifier == identifier && *reply_sequence == sequence
            )
        });

        match received {
            Some(Received::Echo { source_addr, .. }) => Ok(EchoOutcome {
                source_addr,
                round_trip: Instant::now() - sent_at,
                sequence,
            }),
            _ => Err(NetworkError::Unreachable),
        }
    }
}

/// Addressing of a single echo message.
///
/// Only emitted messages are described, so the local host is always the source
/// and the peer always the destination.
#[derive(Debug, Clone, Copy)]
struct EchoTarget {
    local_mac: MacAddress,
    peer_mac: MacAddress,
    local_ip: Ipv4Addr,
    peer_ip: Ipv4Addr,
}

/// Build an echo request or reply into `buffer` and hand it to the driver.
fn emit_echo(
    buffer: &mut [u8; FRAME_BUFFER_LEN],
    nic: &mut dyn Nic,
    msg_type: MessageType,
    target: EchoTarget,
    identifier: u16,
    sequence: u16,
    payload: &[u8],
) -> NetworkResult<()> {
    let envelope = EthernetIpv4Envelope {
        source_hardware_addr: target.local_mac,
        destination_hardware_addr: target.peer_mac,
        source_addr: target.local_ip,
        destination_addr: target.peer_ip,
        protocol: ip::Protocol::Icmp,
        // Overwritten by the echo emitter from the payload length.
        payload_len: 0,
        ttl: DEFAULT_IPV4_TTL,
        flags: ipv4::Flags::new(true, false),
    };

    let emit = match msg_type {
        MessageType::EchoRequest => echo::emit_request,
        MessageType::EchoReply => echo::emit_reply,
        _ => return Err(NetworkError::Unsupported),
    };

    let len = emit(envelope, &mut buffer[..], identifier, sequence, payload)?;

    nic.send_frame(&buffer[..len])
}

/// Bring the interface up on top of the system NIC.
///
/// # Errors
///
/// Returns `Absent` when no NIC driver is available.
pub fn init() -> NetworkResult<()> {
    let mac = nic::with_nic(|nic| nic.mac_address()).ok_or(NetworkError::Absent)?;

    let route = default_route();
    let interface = Interface::new(mac, route)?;

    crate::info!(
        "Network interface up. MAC: {}; IPv4: {}/{}",
        mac,
        route.addr(),
        route.cidr().prefix_len()
    );

    INTERFACE.init(interface);

    Ok(())
}

/// Whether the interface has been brought up.
#[must_use]
#[inline]
pub fn available() -> bool {
    INTERFACE.is_initialized()
}

/// Run `f` against the interface, if it is up.
pub fn with_interface<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Interface) -> R,
{
    INTERFACE.with_locked_if_init(f)
}

/// Send one ICMP echo request to `addr` and wait up to `timeout` for its reply.
///
/// # Errors
///
/// Returns `Uninitialized` when no interface is up, `Absent` when the NIC went
/// away, and `Unreachable` when the next hop cannot be resolved or no reply
/// arrives in time.
pub fn ping(addr: Ipv4Addr, timeout: Duration) -> NetworkResult<EchoOutcome> {
    with_interface(|interface| {
        nic::with_nic(|nic| interface.echo(nic, addr, timeout)).ok_or(NetworkError::Absent)?
    })
    .ok_or(NetworkError::Uninitialized)?
}
