use crate::NetworkError;

pub mod v4;
pub mod v6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
/// IPv4 protocol number.
pub enum Protocol {
    Icmp = 1,
    Igmp = 2,
    Tcp = 6,
    Udp = 17,
}

impl TryFrom<u8> for Protocol {
    type Error = NetworkError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Icmp),
            2 => Ok(Self::Igmp),
            6 => Ok(Self::Tcp),
            17 => Ok(Self::Udp),
            _ => Err(NetworkError::Invalid),
        }
    }
}

impl From<Protocol> for u8 {
    fn from(value: Protocol) -> Self {
        value as Self
    }
}
