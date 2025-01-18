#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub const BROADCAST: Self = Self([0xFF; 6]);

    #[must_use]
    #[inline]
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    #[must_use]
    #[inline]
    pub fn is_broadcast(&self) -> bool {
        self.0 == Self::BROADCAST.0
    }

    #[must_use]
    #[inline]
    pub const fn is_multicast(&self) -> bool {
        self.0[0] & 0b1 == 0b1
    }

    #[must_use]
    #[inline]
    pub fn is_unicast(&self) -> bool {
        !(self.is_multicast() || self.is_broadcast())
    }

    #[must_use]
    #[inline]
    pub const fn is_local(&self) -> bool {
        self.0[0] & 0b10 == 0b10
    }
}

impl core::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Ethernet Ethertype II frame header
pub struct Header {
    mac_dest: [u8; 6],
    mac_src: [u8; 6],
    // vlan_tag: u32,
    ethertype: Ethertype,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
/// See <https://en.wikipedia.org/wiki/EtherType>
pub enum Ethertype {
    Arp = 0x0806,
    Ipv4 = 0x0800,
    Ipv6 = 0x86DD,
}

// Even though `Copy` could be derived, considering the size of the struct,
// we should be using as many references as possible.
// FIXME: How to avoid placing 1500 bytes on the stack? heap (is it fast enough?)?
#[derive(Debug, Clone)]
/// Ethernet frame
pub struct Packet {
    header: Header,
    /// 46-1500 bytes
    data_length: u16,
    data: [u8; 1500],
    crc_checksum: u32,
}

impl Packet {
    #[must_use]
    #[inline]
    pub fn new_arp(data_length: u16, filler: impl FnOnce(&mut [u8; 1500])) -> Self {
        Self::new(data_length, filler, Ethertype::Arp, MacAddress::BROADCAST)
    }

    #[must_use]
    #[inline]
    pub fn new_ipv4(data_length: u16, filler: impl FnOnce(&mut [u8; 1500])) -> Self {
        Self::new(data_length, filler, Ethertype::Ipv4, todo!())
    }

    #[must_use]
    #[inline]
    pub fn new_ipv6(data_length: u16, filler: impl FnOnce(&mut [u8; 1500])) -> Self {
        Self::new(data_length, filler, Ethertype::Ipv6, todo!())
    }

    fn new(
        data_length: u16,
        filler: impl FnOnce(&mut [u8; 1500]),
        ethertype: Ethertype,
        mac_dest: MacAddress,
    ) -> Self {
        assert!(data_length >= 46);
        assert!(data_length <= 1500);

        let mac_src = crate::drivers::nic::with_nic(|nic| nic.mac_address())
            .unwrap()
            .0;

        let header = Header {
            mac_dest: mac_dest.0,
            mac_src,
            ethertype,
        };

        let mut data = [0; 1500];
        filler(&mut data);

        Self {
            header,
            data_length,
            data,
            crc_checksum: 0,
        }
    }

    fn into_raw(self) -> [u8; 1518] {
        let mut raw = [0; 1518];

        raw[..6].copy_from_slice(&self.header.mac_dest);
        raw[6..12].copy_from_slice(&self.header.mac_src);
        raw[12..14].copy_from_slice(&(self.header.ethertype as u16).to_be_bytes());
        raw[14..14 + usize::from(self.data_length)]
            .copy_from_slice(&self.data[..self.data_length as usize]);

        todo!("CRC checksum");

        raw
    }

    pub fn send(&self) {
        todo!("Send packet to NIC");
    }
}
