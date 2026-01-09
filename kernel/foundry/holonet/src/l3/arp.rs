use crate::{
    NetworkError, NetworkResult,
    l2::ethernet::{EtherType, MacAddress},
    l3::ip::Ipv4Addr,
    utils::{u16_from_inet_bytes, u16_to_inet_bytes},
};

/// Range of bytes for the hardware type field.
const HARDWARE_TYPE_RANGE: core::ops::Range<usize> = 0..2;
/// Range of bytes for the protocol type field.
const PROTOCOL_TYPE_RANGE: core::ops::Range<usize> = 2..4;
/// Index of the hardware length field.
const HARDWARE_LEN_IDX: usize = 4;
/// Index of the protocol length field.
const PROTOCOL_LEN_IDX: usize = 5;
/// Range of bytes for the operation field.
const OPERATION_RANGE: core::ops::Range<usize> = 6..8;
/// Length of the fixed-size header (end of the operation field).
const FIXED_HEADER_LEN: usize = 8;

#[must_use]
#[inline]
#[expect(non_snake_case, reason = "Match the other fields names")]
const fn SOURCE_HARDWARE_ADDR_RANGE(hardware_len: u8) -> core::ops::Range<usize> {
    let start = FIXED_HEADER_LEN;
    start..(start + hardware_len as usize)
}

#[must_use]
#[inline]
#[expect(non_snake_case, reason = "Match the other fields names")]
const fn SOURCE_PROTOCOL_ADDR_RANGE(hardware_len: u8, protocol_len: u8) -> core::ops::Range<usize> {
    // End of SHA
    let start = FIXED_HEADER_LEN + hardware_len as usize;
    start..(start + protocol_len as usize)
}

#[must_use]
#[inline]
#[expect(non_snake_case, reason = "Match the other fields names")]
const fn TARGET_HARDWARE_ADDR_RANGE(hardware_len: u8, protocol_len: u8) -> core::ops::Range<usize> {
    // End of SPA
    let start = FIXED_HEADER_LEN + hardware_len as usize + protocol_len as usize;
    start..(start + hardware_len as usize)
}

#[must_use]
#[inline]
#[expect(non_snake_case, reason = "Match the other fields names")]
const fn TARGET_PROTOCOL_ADDR_RANGE(hardware_len: u8, protocol_len: u8) -> core::ops::Range<usize> {
    // End of THA
    let start = FIXED_HEADER_LEN + 2 * hardware_len as usize + protocol_len as usize;
    start..(start + protocol_len as usize)
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
#[repr(u16)]
/// ARP hardware type.
pub enum Hardware {
    Ethernet = 1,
}

impl TryFrom<u16> for Hardware {
    type Error = NetworkError;

    fn try_from(raw: u16) -> Result<Self, Self::Error> {
        match raw {
            1 => Ok(Self::Ethernet),
            _ => Err(NetworkError::Invalid),
        }
    }
}

impl From<Hardware> for u16 {
    #[inline]
    fn from(hw: Hardware) -> Self {
        hw as Self
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
#[repr(u16)]
/// ARP operation type.
pub enum Operation {
    Request = 1,
    Reply = 2,
}

impl TryFrom<u16> for Operation {
    type Error = NetworkError;

    fn try_from(raw: u16) -> Result<Self, Self::Error> {
        match raw {
            1 => Ok(Self::Request),
            2 => Ok(Self::Reply),
            _ => Err(NetworkError::Invalid),
        }
    }
}

impl From<Operation> for u16 {
    #[inline]
    fn from(hw: Operation) -> Self {
        hw as Self
    }
}

/// A read/write wrapper around an Address Resolution `EtherType` packet buffer.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Packet<T: AsRef<[u8]>> {
    buffer: T,
}

impl<T: AsRef<[u8]>> Packet<T> {
    #[must_use]
    #[inline]
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
        let min_len = TARGET_PROTOCOL_ADDR_RANGE(self.hardware_len(), self.protocol_len()).end;
        (self.buffer.as_ref().len() >= min_len)
            .then_some(())
            .ok_or(NetworkError::Invalid)
    }

    #[must_use]
    #[inline]
    pub fn into_inner(self) -> T {
        self.buffer
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the hardware type field.
    pub fn hardware_type(&self) -> Hardware {
        let data = self.buffer.as_ref();
        let raw = u16_from_inet_bytes(data[HARDWARE_TYPE_RANGE].try_into().unwrap());
        Hardware::try_from(raw).unwrap()
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the protocol type field.
    pub fn protocol_type(&self) -> EtherType {
        let data = self.buffer.as_ref();
        let raw = u16_from_inet_bytes(data[PROTOCOL_TYPE_RANGE].try_into().unwrap());
        EtherType::try_from(raw).unwrap()
    }

    #[must_use]
    #[inline]
    /// Return the hardware length field.
    pub fn hardware_len(&self) -> u8 {
        self.buffer.as_ref()[HARDWARE_LEN_IDX]
    }

    #[must_use]
    #[inline]
    /// Return the protocol length field.
    pub fn protocol_len(&self) -> u8 {
        self.buffer.as_ref()[PROTOCOL_LEN_IDX]
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Return the operation field.
    pub fn operation(&self) -> Operation {
        let data = self.buffer.as_ref();
        let raw = u16_from_inet_bytes(data[OPERATION_RANGE].try_into().unwrap());
        Operation::try_from(raw).unwrap()
    }

    #[must_use]
    #[inline]
    /// Return the source hardware address field.
    pub fn source_hardware_addr(&self) -> &[u8] {
        let data = self.buffer.as_ref();
        &data[SOURCE_HARDWARE_ADDR_RANGE(self.hardware_len())]
    }

    #[must_use]
    #[inline]
    /// Return the source protocol address field.
    pub fn source_protocol_addr(&self) -> &[u8] {
        let data = self.buffer.as_ref();
        &data[SOURCE_PROTOCOL_ADDR_RANGE(self.hardware_len(), self.protocol_len())]
    }

    #[must_use]
    #[inline]
    /// Return the target hardware address field.
    pub fn target_hardware_addr(&self) -> &[u8] {
        let data = self.buffer.as_ref();
        &data[TARGET_HARDWARE_ADDR_RANGE(self.hardware_len(), self.protocol_len())]
    }

    #[must_use]
    #[inline]
    /// Return the target protocol address field.
    pub fn target_protocol_addr(&self) -> &[u8] {
        let data = self.buffer.as_ref();
        &data[TARGET_PROTOCOL_ADDR_RANGE(self.hardware_len(), self.protocol_len())]
    }

    #[must_use]
    #[inline]
    /// Return the length of an ARP packet buffer for Ethernet/IPv4.
    pub const fn buffer_len() -> usize {
        const { TARGET_PROTOCOL_ADDR_RANGE(6, 4).end }
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    #[inline]
    /// Set the hardware type field.
    pub fn set_hardware_type(&mut self, value: Hardware) {
        let data = self.buffer.as_mut();
        data[HARDWARE_TYPE_RANGE].copy_from_slice(&u16_to_inet_bytes(value.into()));
    }

    #[inline]
    /// Set the protocol type field.
    pub fn set_protocol_type(&mut self, value: EtherType) {
        let data = self.buffer.as_mut();
        data[PROTOCOL_TYPE_RANGE].copy_from_slice(&u16_to_inet_bytes(value.into()));
    }

    #[inline]
    /// Set the hardware length field.
    pub fn set_hardware_len(&mut self, value: u8) {
        self.buffer.as_mut()[HARDWARE_LEN_IDX] = value;
    }

    #[inline]
    /// Set the protocol length field.
    pub fn set_protocol_len(&mut self, value: u8) {
        self.buffer.as_mut()[PROTOCOL_LEN_IDX] = value;
    }

    #[inline]
    /// Set the operation field.
    pub fn set_operation(&mut self, value: Operation) {
        let data = self.buffer.as_mut();
        data[OPERATION_RANGE].copy_from_slice(&u16_to_inet_bytes(value.into()));
    }

    /// # Panics
    ///
    /// The function panics if `value` is not `self.hardware_len()` long.
    pub fn set_source_hardware_addr(&mut self, value: &[u8]) {
        let hw_len = self.hardware_len(); // Please the borrow checker
        let data = self.buffer.as_mut();
        data[SOURCE_HARDWARE_ADDR_RANGE(hw_len)].copy_from_slice(value);
    }

    /// # Panics
    ///
    /// The function panics if `value` is not `self.protocol_len()` long.
    pub fn set_source_protocol_addr(&mut self, value: &[u8]) {
        let hw_len = self.hardware_len(); // Please the borrow checker
        let proto_len = self.protocol_len(); // Please the borrow checker
        let data = self.buffer.as_mut();
        data[SOURCE_PROTOCOL_ADDR_RANGE(hw_len, proto_len)].copy_from_slice(value);
    }

    /// # Panics
    ///
    /// The function panics if `value` is not `self.hardware_len()` long.
    pub fn set_target_hardware_addr(&mut self, value: &[u8]) {
        let hw_len = self.hardware_len(); // Please the borrow checker
        let proto_len = self.protocol_len(); // Please the borrow checker
        let data = self.buffer.as_mut();
        data[TARGET_HARDWARE_ADDR_RANGE(hw_len, proto_len)].copy_from_slice(value);
    }

    /// # Panics
    ///
    /// The function panics if `value` is not `self.protocol_len()` long.
    pub fn set_target_protocol_addr(&mut self, value: &[u8]) {
        let hw_len = self.hardware_len(); // Please the borrow checker
        let proto_len = self.protocol_len(); // Please the borrow checker
        let data = self.buffer.as_mut();
        data[TARGET_PROTOCOL_ADDR_RANGE(hw_len, proto_len)].copy_from_slice(value);
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for Packet<T> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[non_exhaustive]
/// A high-level representation of an Address Resolution `EtherType` packet.
pub enum Repr {
    /// An Ethernet and IPv4 Address Resolution `EtherType` packet.
    EthernetIpv4 {
        operation: Operation,
        source_hardware_addr: MacAddress,
        source_protocol_addr: Ipv4Addr,
        target_hardware_addr: MacAddress,
        target_protocol_addr: Ipv4Addr,
    },
}

impl Repr {
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Parse an Address Resolution `EtherType` packet and return a high-level representation,
    /// or return `Err(Error)` if the packet is not recognized.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the packet is too short and `Unsupported` if the packet is not recognized.
    pub fn parse<T: AsRef<[u8]>>(packet: &Packet<T>) -> NetworkResult<Self> {
        packet.check_len()?;

        match (
            packet.hardware_type(),
            packet.protocol_type(),
            packet.hardware_len(),
            packet.protocol_len(),
        ) {
            (Hardware::Ethernet, EtherType::IpV4, 6, 4) => Ok(Self::EthernetIpv4 {
                operation: packet.operation(),
                source_hardware_addr: MacAddress::from_bytes(packet.source_hardware_addr()),
                // TODO: When `ip_from` (`Ipv4Addr::from_bytes`) is stable, use it.
                source_protocol_addr: Ipv4Addr::from(
                    <[u8; 4]>::try_from(packet.source_protocol_addr()).unwrap(),
                ),
                target_hardware_addr: MacAddress::from_bytes(packet.target_hardware_addr()),
                // TODO: When `ip_from` (`Ipv4Addr::from_bytes`) is stable, use it.
                target_protocol_addr: Ipv4Addr::from(
                    <[u8; 4]>::try_from(packet.target_protocol_addr()).unwrap(),
                ),
            }),
            _ => Err(NetworkError::Unsupported),
        }
    }

    #[must_use]
    #[inline]
    /// Return the length of a packet that will be emitted from this high-level representation.
    pub const fn buffer_len(&self) -> usize {
        match self {
            &Self::EthernetIpv4 { .. } => const { TARGET_PROTOCOL_ADDR_RANGE(6, 4).end },
        }
    }

    /// Emit a high-level representation into an Address Resolution `EtherType` packet.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) {
        match self {
            &Self::EthernetIpv4 {
                operation,
                source_hardware_addr,
                source_protocol_addr,
                target_hardware_addr,
                target_protocol_addr,
            } => {
                packet.set_hardware_type(Hardware::Ethernet);
                packet.set_protocol_type(EtherType::IpV4);
                packet.set_hardware_len(6);
                packet.set_protocol_len(4);
                packet.set_operation(operation);
                packet.set_source_hardware_addr(&source_hardware_addr.as_bytes());
                packet.set_source_protocol_addr(&source_protocol_addr.octets());
                packet.set_target_hardware_addr(&target_hardware_addr.as_bytes());
                packet.set_target_protocol_addr(&target_protocol_addr.octets());
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec;

    const PACKET_BYTES: [u8; 28] = [
        0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x21,
        0x22, 0x23, 0x24, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x41, 0x42, 0x43, 0x44,
    ];

    #[test]
    fn test_deconstruct() {
        let packet = Packet::new_unchecked(PACKET_BYTES);
        assert_eq!(packet.hardware_type(), Hardware::Ethernet);
        assert_eq!(packet.protocol_type(), EtherType::IpV4);
        assert_eq!(packet.hardware_len(), 6);
        assert_eq!(packet.protocol_len(), 4);
        assert_eq!(packet.operation(), Operation::Request);
        assert_eq!(
            packet.source_hardware_addr(),
            &[0x11, 0x12, 0x13, 0x14, 0x15, 0x16]
        );
        assert_eq!(packet.source_protocol_addr(), &[0x21, 0x22, 0x23, 0x24]);
        assert_eq!(
            packet.target_hardware_addr(),
            &[0x31, 0x32, 0x33, 0x34, 0x35, 0x36]
        );
        assert_eq!(packet.target_protocol_addr(), &[0x41, 0x42, 0x43, 0x44]);
    }

    #[test]
    fn test_construct() {
        let mut bytes = vec![0xa5; 28];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_hardware_type(Hardware::Ethernet);
        packet.set_protocol_type(EtherType::IpV4);
        packet.set_hardware_len(6);
        packet.set_protocol_len(4);
        packet.set_operation(Operation::Request);
        packet.set_source_hardware_addr(&[0x11, 0x12, 0x13, 0x14, 0x15, 0x16]);
        packet.set_source_protocol_addr(&[0x21, 0x22, 0x23, 0x24]);
        packet.set_target_hardware_addr(&[0x31, 0x32, 0x33, 0x34, 0x35, 0x36]);
        packet.set_target_protocol_addr(&[0x41, 0x42, 0x43, 0x44]);
        assert_eq!(&*packet.into_inner(), &PACKET_BYTES);
    }

    // TODO: When `ip_from` (`Ipv4Addr::from_bytes`) is stable, replace this with a const.
    fn expected_packet_repr() -> Repr {
        Repr::EthernetIpv4 {
            operation: Operation::Request,
            source_hardware_addr: MacAddress::from_bytes(&[0x11, 0x12, 0x13, 0x14, 0x15, 0x16]),
            source_protocol_addr: Ipv4Addr::from([0x21, 0x22, 0x23, 0x24]),
            target_hardware_addr: MacAddress::from_bytes(&[0x31, 0x32, 0x33, 0x34, 0x35, 0x36]),
            target_protocol_addr: Ipv4Addr::from([0x41, 0x42, 0x43, 0x44]),
        }
    }

    #[test]
    fn test_parse() {
        let packet = Packet::new_unchecked(PACKET_BYTES);
        let repr = Repr::parse(&packet).unwrap();
        assert_eq!(repr, expected_packet_repr());
    }

    #[test]
    fn test_emit() {
        let mut bytes = vec![0xa5; 28];
        let mut packet = Packet::new_unchecked(&mut bytes);
        expected_packet_repr().emit(&mut packet);
        assert_eq!(&*packet.into_inner(), &PACKET_BYTES);
    }
}
