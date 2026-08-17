//! IPv4 addressing and next-hop selection.
//!
//! Deciding where a frame has to be sent is protocol logic rather than driver
//! logic: a destination on the directly attached link is reached on its own,
//! anything else has to go through a router. This module owns that decision so
//! that the interface layer only has to resolve the address it is handed.

use crate::{NetworkError, NetworkResult, l3::ip::v4::Ipv4Addr};

/// The maximum length of an IPv4 network prefix, in bits.
const MAX_PREFIX_LEN: u8 = 32;

/// An IPv4 address together with the length of its network prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Cidr {
    addr: Ipv4Addr,
    prefix_len: u8,
}

impl Ipv4Cidr {
    /// Build an address/prefix pair.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if `prefix_len` exceeds 32.
    pub const fn new(addr: Ipv4Addr, prefix_len: u8) -> NetworkResult<Self> {
        if prefix_len > MAX_PREFIX_LEN {
            return Err(NetworkError::Invalid);
        }

        Ok(Self { addr, prefix_len })
    }

    #[must_use]
    #[inline]
    pub const fn addr(self) -> Ipv4Addr {
        self.addr
    }

    #[must_use]
    #[inline]
    pub const fn prefix_len(self) -> u8 {
        self.prefix_len
    }

    /// Return the network mask as a bit pattern.
    #[must_use]
    #[inline]
    pub const fn netmask(self) -> u32 {
        // A zero-length prefix would shift by 32, which is undefined for u32.
        if self.prefix_len == 0 {
            0
        } else {
            u32::MAX << (MAX_PREFIX_LEN - self.prefix_len)
        }
    }

    /// Return the network address of the prefix.
    #[must_use]
    #[inline]
    pub const fn network(self) -> Ipv4Addr {
        Ipv4Addr::from_bits(self.addr.to_bits() & self.netmask())
    }

    /// Return the directed broadcast address of the prefix.
    #[must_use]
    #[inline]
    pub const fn broadcast(self) -> Ipv4Addr {
        Ipv4Addr::from_bits(self.addr.to_bits() | !self.netmask())
    }

    /// Whether `addr` belongs to this prefix.
    #[must_use]
    #[inline]
    pub const fn contains(self, addr: Ipv4Addr) -> bool {
        let mask = self.netmask();
        (addr.to_bits() & mask) == (self.addr.to_bits() & mask)
    }
}

/// The routing state of a single IPv4 interface.
///
/// This is deliberately a single prefix plus an optional default router rather
/// than a routing table: it is what a host needs to reach both its own link and
/// the rest of the internet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Route {
    cidr: Ipv4Cidr,
    gateway: Option<Ipv4Addr>,
}

impl Ipv4Route {
    /// Build the routing state from a local prefix and an optional router.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the gateway does not sit on the local prefix, as
    /// there would then be no way to reach it.
    pub const fn new(cidr: Ipv4Cidr, gateway: Option<Ipv4Addr>) -> NetworkResult<Self> {
        if let Some(gateway) = gateway
            && !cidr.contains(gateway)
        {
            return Err(NetworkError::Invalid);
        }

        Ok(Self { cidr, gateway })
    }

    #[must_use]
    #[inline]
    pub const fn cidr(self) -> Ipv4Cidr {
        self.cidr
    }

    #[must_use]
    #[inline]
    pub const fn addr(self) -> Ipv4Addr {
        self.cidr.addr()
    }

    #[must_use]
    #[inline]
    pub const fn gateway(self) -> Option<Ipv4Addr> {
        self.gateway
    }

    /// Return the address a frame has to be addressed to in order to reach
    /// `destination`.
    ///
    /// On-link destinations are their own next hop; everything else is handed
    /// to the gateway.
    ///
    /// # Errors
    ///
    /// Returns `Unreachable` if `destination` is off-link and no gateway is
    /// configured.
    pub const fn next_hop(self, destination: Ipv4Addr) -> NetworkResult<Ipv4Addr> {
        if self.cidr.contains(destination) {
            return Ok(destination);
        }

        match self.gateway {
            Some(gateway) => Ok(gateway),
            None => Err(NetworkError::Unreachable),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const LOCAL: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 15);
    const GATEWAY: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 1);

    fn cidr(prefix_len: u8) -> Ipv4Cidr {
        Ipv4Cidr::new(LOCAL, prefix_len).unwrap()
    }

    fn route() -> Ipv4Route {
        Ipv4Route::new(cidr(24), Some(GATEWAY)).unwrap()
    }

    #[test]
    fn test_cidr_rejects_oversized_prefix() {
        assert_eq!(Ipv4Cidr::new(LOCAL, 33), Err(NetworkError::Invalid));
        assert!(Ipv4Cidr::new(LOCAL, 32).is_ok());
    }

    #[test]
    fn test_netmask_covers_the_prefix_bounds() {
        // A zero-length prefix must not shift a u32 by 32.
        assert_eq!(cidr(0).netmask(), 0);
        assert_eq!(cidr(8).netmask(), 0xFF00_0000);
        assert_eq!(cidr(24).netmask(), 0xFFFF_FF00);
        assert_eq!(cidr(32).netmask(), u32::MAX);
    }

    #[test]
    fn test_network_and_broadcast() {
        assert_eq!(cidr(24).network(), Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(cidr(24).broadcast(), Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(cidr(32).network(), LOCAL);
        assert_eq!(cidr(32).broadcast(), LOCAL);
    }

    #[test]
    fn test_contains_only_matches_the_prefix() {
        let cidr = cidr(24);

        assert!(cidr.contains(Ipv4Addr::new(192, 168, 1, 1)));
        assert!(cidr.contains(Ipv4Addr::new(192, 168, 1, 255)));
        assert!(!cidr.contains(Ipv4Addr::new(192, 168, 2, 1)));
        assert!(!cidr.contains(Ipv4Addr::new(1, 1, 1, 1)));
    }

    #[test]
    fn test_zero_length_prefix_contains_everything() {
        let cidr = cidr(0);

        assert!(cidr.contains(Ipv4Addr::new(1, 1, 1, 1)));
        assert!(cidr.contains(Ipv4Addr::new(255, 0, 0, 1)));
    }

    #[test]
    fn test_route_rejects_off_link_gateway() {
        assert_eq!(
            Ipv4Route::new(cidr(24), Some(Ipv4Addr::new(10, 0, 0, 1))),
            Err(NetworkError::Invalid)
        );
    }

    #[test]
    fn test_on_link_destination_is_its_own_next_hop() {
        let destination = Ipv4Addr::new(192, 168, 1, 200);
        assert_eq!(route().next_hop(destination), Ok(destination));
    }

    #[test]
    fn test_off_link_destination_goes_through_the_gateway() {
        assert_eq!(route().next_hop(Ipv4Addr::new(1, 1, 1, 1)), Ok(GATEWAY));
    }

    #[test]
    fn test_off_link_destination_without_gateway_is_unreachable() {
        let route = Ipv4Route::new(cidr(24), None).unwrap();

        assert_eq!(
            route.next_hop(Ipv4Addr::new(1, 1, 1, 1)),
            Err(NetworkError::Unreachable)
        );
        // An on-link destination still resolves without a gateway.
        assert_eq!(
            route.next_hop(Ipv4Addr::new(192, 168, 1, 2)),
            Ok(Ipv4Addr::new(192, 168, 1, 2))
        );
    }
}
