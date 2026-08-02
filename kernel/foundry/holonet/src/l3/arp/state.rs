use hyperdrive::queues::lru::{Entry, InsertResult, LruCache, hash};

use super::{Operation, Packet, Repr};
use crate::{
    NetworkError, NetworkResult,
    l2::ethernet::{self, EtherType, MacAddress},
    l3::ip::v4::Ipv4Addr,
    utils::ensure_len,
};

/// A cached Ethernet/IPv4 ARP mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheEntry {
    pub protocol_addr: Ipv4Addr,
    pub hardware_addr: MacAddress,
    pub expires_at: u64,
}

/// Result of inserting or refreshing an ARP cache mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheInsertResult {
    Added,
    Refreshed { previous: CacheEntry },
    Evicted { evicted: CacheEntry },
}

/// A fixed-capacity ARP cache for Ethernet/IPv4 neighbors.
#[derive(Debug)]
pub struct Cache<const CAPACITY: usize, const BUCKETS: usize> {
    entries: LruCache<Ipv4Addr, MacAddress, CAPACITY, BUCKETS, hash::FnvBuildHasher, u64>,
    ttl_ticks: u64,
    now: u64,
}

impl<const CAPACITY: usize, const BUCKETS: usize> Cache<CAPACITY, BUCKETS> {
    /// Create a cache with a caller-managed logical TTL.
    pub fn new(ttl_ticks: u64) -> NetworkResult<Self> {
        if CAPACITY == 0 || BUCKETS == 0 || ttl_ticks == 0 {
            return Err(NetworkError::Invalid);
        }

        Ok(Self {
            entries: LruCache::default(),
            ttl_ticks,
            now: 0,
        })
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    #[inline]
    pub const fn ttl_ticks(&self) -> u64 {
        self.ttl_ticks
    }

    #[must_use]
    #[inline]
    pub const fn now(&self) -> u64 {
        self.now
    }

    #[inline]
    const fn expires_at(&self) -> u64 {
        self.now.saturating_add(self.ttl_ticks)
    }

    #[must_use]
    #[inline]
    const fn is_expired(&self, expires_at: u64) -> bool {
        expires_at <= self.now
    }

    /// Advance the logical clock and purge stale entries.
    pub fn advance(&mut self, ticks: u64) {
        self.now = self.now.saturating_add(ticks);
        let now = self.now;
        self.entries.retain(|entry| *entry.meta() > now);
    }

    /// Insert or refresh a mapping.
    pub fn insert(
        &mut self,
        protocol_addr: Ipv4Addr,
        hardware_addr: MacAddress,
    ) -> CacheInsertResult {
        match self
            .entries
            .insert_with_meta(protocol_addr, hardware_addr, self.expires_at())
        {
            InsertResult::Added => CacheInsertResult::Added,
            InsertResult::Replaced { value, meta } => CacheInsertResult::Refreshed {
                previous: CacheEntry {
                    protocol_addr,
                    hardware_addr: value,
                    expires_at: meta,
                },
            },
            InsertResult::Evicted(evicted) => {
                let evicted = cache_entry_from_lru(&evicted);
                CacheInsertResult::Evicted { evicted }
            }
        }
    }

    /// Resolve a mapping, touching it in the LRU if it is still valid.
    pub fn resolve(&mut self, protocol_addr: Ipv4Addr) -> Option<MacAddress> {
        let entry = self.entries.peek_entry(&protocol_addr)?;
        let expired = self.is_expired(*entry.meta());

        if expired {
            let _ = self.entries.remove(&protocol_addr);
            return None;
        }

        self.entries.get(&protocol_addr).copied()
    }

    /// Remove a mapping from the cache.
    pub fn remove(&mut self, protocol_addr: Ipv4Addr) -> Option<CacheEntry> {
        self.entries
            .remove(&protocol_addr)
            .map(|entry| cache_entry_from_lru(&entry))
    }
}

const fn cache_entry_from_lru(entry: &Entry<Ipv4Addr, MacAddress, u64>) -> CacheEntry {
    CacheEntry {
        protocol_addr: entry.key,
        hardware_addr: entry.value,
        expires_at: entry.meta,
    }
}

/// A complete Ethernet II + ARP frame representation for Ethernet/IPv4 ARP traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthernetIpv4Frame {
    pub ethernet: ethernet::Repr,
    pub arp: Repr,
}

impl EthernetIpv4Frame {
    const ZERO_HARDWARE_ADDR: MacAddress = MacAddress::new([0; 6]);

    #[must_use]
    #[inline]
    pub const fn buffer_len(&self) -> usize {
        ethernet::HEADER_LEN + self.arp.buffer_len()
    }

    #[must_use]
    pub const fn request(
        source_hardware_addr: MacAddress,
        source_protocol_addr: Ipv4Addr,
        target_protocol_addr: Ipv4Addr,
    ) -> Self {
        Self {
            ethernet: ethernet::Repr {
                src_addr: source_hardware_addr,
                dst_addr: MacAddress::BROADCAST,
                ethertype: EtherType::Arp,
            },
            arp: Repr::EthernetIpv4 {
                operation: Operation::Request,
                source_hardware_addr,
                source_protocol_addr,
                target_hardware_addr: Self::ZERO_HARDWARE_ADDR,
                target_protocol_addr,
            },
        }
    }

    #[must_use]
    pub const fn reply(
        source_hardware_addr: MacAddress,
        source_protocol_addr: Ipv4Addr,
        target_hardware_addr: MacAddress,
        target_protocol_addr: Ipv4Addr,
    ) -> Self {
        Self {
            ethernet: ethernet::Repr {
                src_addr: source_hardware_addr,
                dst_addr: target_hardware_addr,
                ethertype: EtherType::Arp,
            },
            arp: Repr::EthernetIpv4 {
                operation: Operation::Reply,
                source_hardware_addr,
                source_protocol_addr,
                target_hardware_addr,
                target_protocol_addr,
            },
        }
    }

    /// Emit the frame into a raw octet buffer.
    pub fn emit(&self, buffer: &mut [u8]) -> NetworkResult<()> {
        ensure_len(buffer, self.buffer_len())?;

        let (frame_bytes, _) = buffer.split_at_mut(self.buffer_len());
        let mut frame = ethernet::Frame::new(frame_bytes)?;
        self.ethernet.emit(&mut frame)?;

        let payload = frame.payload_mut()?;
        let mut packet = Packet::new(payload)?;
        self.arp.emit(&mut packet)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ingress::{EthernetPayload, IngressFrame};

    #[test]
    fn test_cache_evicts_least_recently_used_entry() {
        let mut cache = Cache::<2, 2>::new(10).unwrap();
        assert_eq!(
            cache.insert(
                Ipv4Addr::new(10, 0, 0, 1),
                MacAddress::new([0, 1, 2, 3, 4, 5])
            ),
            CacheInsertResult::Added
        );
        assert_eq!(
            cache.insert(
                Ipv4Addr::new(10, 0, 0, 2),
                MacAddress::new([0, 1, 2, 3, 4, 6])
            ),
            CacheInsertResult::Added
        );

        assert_eq!(
            cache.resolve(Ipv4Addr::new(10, 0, 0, 1)),
            Some(MacAddress::new([0, 1, 2, 3, 4, 5]))
        );

        assert_eq!(
            cache.insert(
                Ipv4Addr::new(10, 0, 0, 3),
                MacAddress::new([0, 1, 2, 3, 4, 7])
            ),
            CacheInsertResult::Evicted {
                evicted: CacheEntry {
                    protocol_addr: Ipv4Addr::new(10, 0, 0, 2),
                    hardware_addr: MacAddress::new([0, 1, 2, 3, 4, 6]),
                    expires_at: 10,
                },
            }
        );
    }

    #[test]
    fn test_cache_expires_entries_when_advanced() {
        let mut cache = Cache::<2, 2>::new(3).unwrap();
        let _ = cache.insert(
            Ipv4Addr::new(192, 168, 0, 1),
            MacAddress::new([1, 2, 3, 4, 5, 6]),
        );

        cache.advance(2);
        assert_eq!(
            cache.resolve(Ipv4Addr::new(192, 168, 0, 1)),
            Some(MacAddress::new([1, 2, 3, 4, 5, 6]))
        );

        cache.advance(2);
        assert_eq!(cache.resolve(Ipv4Addr::new(192, 168, 0, 1)), None);
    }

    #[test]
    fn test_emit_request_frame_round_trips_through_ingress() {
        let repr = EthernetIpv4Frame::request(
            MacAddress::new([0x10, 0x11, 0x12, 0x13, 0x14, 0x15]),
            Ipv4Addr::new(192, 168, 0, 10),
            Ipv4Addr::new(192, 168, 0, 1),
        );

        let mut bytes = [0u8; ethernet::HEADER_LEN + Packet::<&[u8]>::buffer_len()];
        repr.emit(&mut bytes).unwrap();

        let ingress = IngressFrame::parse(&bytes).unwrap();
        match ingress.payload {
            EthernetPayload::Arp(packet) => {
                let arp = Repr::parse(&packet).unwrap();
                assert_eq!(
                    arp,
                    EthernetIpv4Frame::request(
                        MacAddress::new([0x10, 0x11, 0x12, 0x13, 0x14, 0x15]),
                        Ipv4Addr::new(192, 168, 0, 10),
                        Ipv4Addr::new(192, 168, 0, 1),
                    )
                    .arp
                );
            }
            _ => panic!("expected ARP ingress frame"),
        }
    }
}
