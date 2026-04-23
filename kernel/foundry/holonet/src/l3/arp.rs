use crate::{
    NetworkError, NetworkResult,
    l2::ethernet::{EtherType, MacAddress},
    l3::ip::v4::Ipv4Addr,
    utils::{ensure_len, read_u8, read_u16, slice, write_slice, write_u8, write_u16},
};

/// Range of bytes for the hardware type field.
const HARDWARE_TYPE: usize = 0;
/// Range of bytes for the protocol type field.
const PROTOCOL_TYPE: usize = 2;
/// Index of the hardware length field.
const HARDWARE_LEN_IDX: usize = 4;
/// Index of the protocol length field.
const PROTOCOL_LEN_IDX: usize = 5;
/// Range of bytes for the operation field.
const OPERATION: usize = 6;
/// Length of the fixed-size header (end of the operation field).
const FIXED_HEADER_LEN: usize = 8;

#[must_use]
#[inline]
const fn source_hardware_addr_range(hardware_len: u8) -> core::ops::Range<usize> {
    let start = FIXED_HEADER_LEN;
    start..(start + hardware_len as usize)
}

#[must_use]
#[inline]
const fn source_protocol_addr_range(hardware_len: u8, protocol_len: u8) -> core::ops::Range<usize> {
    let start = FIXED_HEADER_LEN + hardware_len as usize;
    start..(start + protocol_len as usize)
}

#[must_use]
#[inline]
const fn target_hardware_addr_range(hardware_len: u8, protocol_len: u8) -> core::ops::Range<usize> {
    let start = FIXED_HEADER_LEN + hardware_len as usize + protocol_len as usize;
    start..(start + hardware_len as usize)
}

#[must_use]
#[inline]
const fn target_protocol_addr_range(hardware_len: u8, protocol_len: u8) -> core::ops::Range<usize> {
    let start = FIXED_HEADER_LEN + 2 * hardware_len as usize + protocol_len as usize;
    start..(start + protocol_len as usize)
}

#[must_use]
#[inline]
const fn packet_len(hardware_len: u8, protocol_len: u8) -> usize {
    target_protocol_addr_range(hardware_len, protocol_len).end
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
#[repr(u16)]
#[non_exhaustive]
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
    fn from(op: Operation) -> Self {
        op as Self
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
    /// Returns `Truncated` if the buffer is too short.
    pub fn new(buffer: T) -> NetworkResult<Self> {
        let packet = Self::new_unchecked(buffer);
        packet.check_len()?;
        Ok(packet)
    }

    /// # Errors
    ///
    /// Returns `Truncated` if the buffer is too short for the declared address sizes.
    pub fn check_len(&self) -> NetworkResult<()> {
        ensure_len(
            self.buffer.as_ref(),
            packet_len(self.hardware_len()?, self.protocol_len()?),
        )
    }

    #[must_use]
    #[inline]
    pub fn into_inner(self) -> T {
        self.buffer
    }

    #[must_use]
    #[inline]
    /// Return the raw hardware type field.
    pub fn hardware_type_raw(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), HARDWARE_TYPE)
    }

    #[must_use]
    #[inline]
    /// Return the hardware type field.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the field value is not recognized.
    pub fn hardware_type(&self) -> NetworkResult<Hardware> {
        Hardware::try_from(self.hardware_type_raw()?)
    }

    #[must_use]
    #[inline]
    /// Return the raw protocol type field.
    pub fn protocol_type_raw(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), PROTOCOL_TYPE)
    }

    #[must_use]
    #[inline]
    /// Return the protocol type field.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the field value is not recognized.
    pub fn protocol_type(&self) -> NetworkResult<EtherType> {
        EtherType::try_from(self.protocol_type_raw()?)
    }

    #[must_use]
    #[inline]
    /// Return the hardware length field.
    pub fn hardware_len(&self) -> NetworkResult<u8> {
        read_u8(self.buffer.as_ref(), HARDWARE_LEN_IDX)
    }

    #[must_use]
    #[inline]
    /// Return the protocol length field.
    pub fn protocol_len(&self) -> NetworkResult<u8> {
        read_u8(self.buffer.as_ref(), PROTOCOL_LEN_IDX)
    }

    #[must_use]
    #[inline]
    /// Return the raw operation field.
    pub fn operation_raw(&self) -> NetworkResult<u16> {
        read_u16(self.buffer.as_ref(), OPERATION)
    }

    #[must_use]
    #[inline]
    /// Return the operation field.
    ///
    /// # Errors
    ///
    /// Returns `Invalid` if the field value is not recognized.
    pub fn operation(&self) -> NetworkResult<Operation> {
        Operation::try_from(self.operation_raw()?)
    }

    #[must_use]
    #[inline]
    /// Return the source hardware address field.
    pub fn source_hardware_addr(&self) -> NetworkResult<&[u8]> {
        slice(
            self.buffer.as_ref(),
            source_hardware_addr_range(self.hardware_len()?),
        )
    }

    #[must_use]
    #[inline]
    /// Return the source protocol address field.
    pub fn source_protocol_addr(&self) -> NetworkResult<&[u8]> {
        slice(
            self.buffer.as_ref(),
            source_protocol_addr_range(self.hardware_len()?, self.protocol_len()?),
        )
    }

    #[must_use]
    #[inline]
    /// Return the target hardware address field.
    pub fn target_hardware_addr(&self) -> NetworkResult<&[u8]> {
        slice(
            self.buffer.as_ref(),
            target_hardware_addr_range(self.hardware_len()?, self.protocol_len()?),
        )
    }

    #[must_use]
    #[inline]
    /// Return the target protocol address field.
    pub fn target_protocol_addr(&self) -> NetworkResult<&[u8]> {
        slice(
            self.buffer.as_ref(),
            target_protocol_addr_range(self.hardware_len()?, self.protocol_len()?),
        )
    }

    #[must_use]
    #[inline]
    /// Return the length of an ARP packet buffer for Ethernet/IPv4.
    pub const fn buffer_len() -> usize {
        packet_len(6, 4)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Packet<T> {
    #[inline]
    /// Set the hardware type field.
    pub fn set_hardware_type(&mut self, value: Hardware) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), HARDWARE_TYPE, value.into())
    }

    #[inline]
    /// Set the protocol type field.
    pub fn set_protocol_type(&mut self, value: EtherType) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), PROTOCOL_TYPE, value.into())
    }

    #[inline]
    /// Set the hardware length field.
    pub fn set_hardware_len(&mut self, value: u8) -> NetworkResult<()> {
        write_u8(self.buffer.as_mut(), HARDWARE_LEN_IDX, value)
    }

    #[inline]
    /// Set the protocol length field.
    pub fn set_protocol_len(&mut self, value: u8) -> NetworkResult<()> {
        write_u8(self.buffer.as_mut(), PROTOCOL_LEN_IDX, value)
    }

    #[inline]
    /// Set the operation field.
    pub fn set_operation(&mut self, value: Operation) -> NetworkResult<()> {
        write_u16(self.buffer.as_mut(), OPERATION, value.into())
    }

    /// # Errors
    ///
    /// Returns `Invalid` if `value` does not match the configured hardware length.
    pub fn set_source_hardware_addr(&mut self, value: &[u8]) -> NetworkResult<()> {
        let hw_len = self.hardware_len()?;
        if value.len() != usize::from(hw_len) {
            return Err(NetworkError::Invalid);
        }
        write_slice(
            self.buffer.as_mut(),
            source_hardware_addr_range(hw_len).start,
            value,
        )
    }

    /// # Errors
    ///
    /// Returns `Invalid` if `value` does not match the configured protocol length.
    pub fn set_source_protocol_addr(&mut self, value: &[u8]) -> NetworkResult<()> {
        let hw_len = self.hardware_len()?;
        let proto_len = self.protocol_len()?;
        if value.len() != usize::from(proto_len) {
            return Err(NetworkError::Invalid);
        }
        write_slice(
            self.buffer.as_mut(),
            source_protocol_addr_range(hw_len, proto_len).start,
            value,
        )
    }

    /// # Errors
    ///
    /// Returns `Invalid` if `value` does not match the configured hardware length.
    pub fn set_target_hardware_addr(&mut self, value: &[u8]) -> NetworkResult<()> {
        let hw_len = self.hardware_len()?;
        let proto_len = self.protocol_len()?;
        if value.len() != usize::from(hw_len) {
            return Err(NetworkError::Invalid);
        }
        write_slice(
            self.buffer.as_mut(),
            target_hardware_addr_range(hw_len, proto_len).start,
            value,
        )
    }

    /// # Errors
    ///
    /// Returns `Invalid` if `value` does not match the configured protocol length.
    pub fn set_target_protocol_addr(&mut self, value: &[u8]) -> NetworkResult<()> {
        let hw_len = self.hardware_len()?;
        let proto_len = self.protocol_len()?;
        if value.len() != usize::from(proto_len) {
            return Err(NetworkError::Invalid);
        }
        write_slice(
            self.buffer.as_mut(),
            target_protocol_addr_range(hw_len, proto_len).start,
            value,
        )
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
    /// Parse an Address Resolution `EtherType` packet and return a high-level representation.
    ///
    /// # Errors
    ///
    /// Returns `Truncated` if the packet is too short and `Unsupported` if the packet is not recognized.
    pub fn parse<T: AsRef<[u8]>>(packet: &Packet<T>) -> NetworkResult<Self> {
        packet.check_len()?;

        if packet.hardware_len()? != 6 || packet.protocol_len()? != 4 {
            return Err(NetworkError::Unsupported);
        }

        if packet.hardware_type_raw()? != u16::from(Hardware::Ethernet)
            || packet.protocol_type_raw()? != u16::from(EtherType::IpV4)
        {
            return Err(NetworkError::Unsupported);
        }

        let source_protocol_addr = Ipv4Addr::from(
            <[u8; 4]>::try_from(packet.source_protocol_addr()?)
                .map_err(|_| NetworkError::Invalid)?,
        );
        let target_protocol_addr = Ipv4Addr::from(
            <[u8; 4]>::try_from(packet.target_protocol_addr()?)
                .map_err(|_| NetworkError::Invalid)?,
        );

        Ok(Self::EthernetIpv4 {
            operation: packet.operation()?,
            source_hardware_addr: MacAddress::from_bytes(packet.source_hardware_addr()?)?,
            source_protocol_addr,
            target_hardware_addr: MacAddress::from_bytes(packet.target_hardware_addr()?)?,
            target_protocol_addr,
        })
    }

    #[must_use]
    #[inline]
    /// Return the length of a packet that will be emitted from this high-level representation.
    pub const fn buffer_len(&self) -> usize {
        match self {
            &Self::EthernetIpv4 { .. } => packet_len(6, 4),
        }
    }

    /// Emit a high-level representation into an Address Resolution `EtherType` packet.
    ///
    /// # Errors
    ///
    /// Returns `Truncated` if the packet buffer is too short.
    pub fn emit<T: AsRef<[u8]> + AsMut<[u8]>>(&self, packet: &mut Packet<T>) -> NetworkResult<()> {
        ensure_len(packet.buffer.as_ref(), self.buffer_len())?;
        match self {
            &Self::EthernetIpv4 {
                operation,
                source_hardware_addr,
                source_protocol_addr,
                target_hardware_addr,
                target_protocol_addr,
            } => {
                packet.set_hardware_type(Hardware::Ethernet)?;
                packet.set_protocol_type(EtherType::IpV4)?;
                packet.set_hardware_len(6)?;
                packet.set_protocol_len(4)?;
                packet.set_operation(operation)?;
                packet.set_source_hardware_addr(&source_hardware_addr.as_bytes())?;
                packet.set_source_protocol_addr(&source_protocol_addr.octets())?;
                packet.set_target_hardware_addr(&target_hardware_addr.as_bytes())?;
                packet.set_target_protocol_addr(&target_protocol_addr.octets())
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
        assert_eq!(packet.hardware_type(), Ok(Hardware::Ethernet));
        assert_eq!(packet.protocol_type(), Ok(EtherType::IpV4));
        assert_eq!(packet.hardware_len(), Ok(6));
        assert_eq!(packet.protocol_len(), Ok(4));
        assert_eq!(packet.operation(), Ok(Operation::Request));
        assert_eq!(
            packet.source_hardware_addr(),
            Ok(&[0x11, 0x12, 0x13, 0x14, 0x15, 0x16][..])
        );
        assert_eq!(
            packet.source_protocol_addr(),
            Ok(&[0x21, 0x22, 0x23, 0x24][..])
        );
        assert_eq!(
            packet.target_hardware_addr(),
            Ok(&[0x31, 0x32, 0x33, 0x34, 0x35, 0x36][..])
        );
        assert_eq!(
            packet.target_protocol_addr(),
            Ok(&[0x41, 0x42, 0x43, 0x44][..])
        );
    }

    #[test]
    fn test_construct() {
        let mut bytes = vec![0xa5; 28];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_hardware_type(Hardware::Ethernet).unwrap();
        packet.set_protocol_type(EtherType::IpV4).unwrap();
        packet.set_hardware_len(6).unwrap();
        packet.set_protocol_len(4).unwrap();
        packet.set_operation(Operation::Request).unwrap();
        packet
            .set_source_hardware_addr(&[0x11, 0x12, 0x13, 0x14, 0x15, 0x16])
            .unwrap();
        packet
            .set_source_protocol_addr(&[0x21, 0x22, 0x23, 0x24])
            .unwrap();
        packet
            .set_target_hardware_addr(&[0x31, 0x32, 0x33, 0x34, 0x35, 0x36])
            .unwrap();
        packet
            .set_target_protocol_addr(&[0x41, 0x42, 0x43, 0x44])
            .unwrap();
        assert_eq!(&*packet.into_inner(), &PACKET_BYTES);
    }

    fn expected_packet_repr() -> Repr {
        Repr::EthernetIpv4 {
            operation: Operation::Request,
            source_hardware_addr: MacAddress::from_bytes(&[0x11, 0x12, 0x13, 0x14, 0x15, 0x16])
                .unwrap(),
            source_protocol_addr: Ipv4Addr::from([0x21, 0x22, 0x23, 0x24]),
            target_hardware_addr: MacAddress::from_bytes(&[0x31, 0x32, 0x33, 0x34, 0x35, 0x36])
                .unwrap(),
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
        expected_packet_repr().emit(&mut packet).unwrap();
        assert_eq!(&*packet.into_inner(), &PACKET_BYTES);
    }

    #[test]
    fn test_check_len_with_truncated_fixed_header() {
        let packet = Packet::new_unchecked([0u8; 6]);
        assert_eq!(packet.check_len(), Err(NetworkError::Truncated));
    }

    #[test]
    fn test_parse_unsupported_hw_type() {
        let mut bytes = PACKET_BYTES;
        bytes[0] = 0x00;
        bytes[1] = 0x02;
        let packet = Packet::new_unchecked(bytes);
        assert_eq!(Repr::parse(&packet), Err(NetworkError::Unsupported));
    }

    #[test]
    fn test_set_source_addr_rejects_wrong_length() {
        let mut bytes = vec![0; Packet::<&[u8]>::buffer_len()];
        let mut packet = Packet::new_unchecked(&mut bytes);
        packet.set_hardware_len(6).unwrap();
        assert_eq!(
            packet.set_source_hardware_addr(&[0, 1, 2]),
            Err(NetworkError::Invalid)
        );
    }
}
