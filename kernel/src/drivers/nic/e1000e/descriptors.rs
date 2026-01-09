//! E1000e descriptors structures and implementation.

use beskar_core::arch::PhysAddr;

/// Receive descriptor for the e1000e NIC.
#[repr(C, packed)]
pub struct RxDescriptor {
    buffer_addr: PhysAddr,
    length: u16,
    checksum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

impl RxDescriptor {
    /// Descriptor Done - Hardware has finished processing this descriptor
    const STATUS_DD: u8 = 1 << 0;
    /// End of Packet - This descriptor contains the end of a packet
    const STATUS_EOP: u8 = 1 << 1;

    /// Create a new RX descriptor with the given buffer address.
    #[must_use]
    #[inline]
    pub const fn new(buffer_addr: PhysAddr, length: u16) -> Self {
        Self {
            buffer_addr,
            length,
            checksum: 0,
            status: 0,
            errors: 0,
            special: 0,
        }
    }

    /// Check if the hardware has finished processing this descriptor.
    #[must_use]
    #[inline]
    pub const fn is_done(&self) -> bool {
        self.status & Self::STATUS_DD != 0
    }

    /// Check if this descriptor contains the end of a packet.
    #[must_use]
    #[inline]
    pub const fn is_end_of_packet(&self) -> bool {
        self.status & Self::STATUS_EOP != 0
    }

    /// Get the length of the received packet.
    #[must_use]
    #[inline]
    pub const fn packet_length(&self) -> u16 {
        self.length
    }

    /// Check if there are any errors in the received packet.
    #[must_use]
    #[inline]
    pub const fn has_errors(&self) -> bool {
        self.errors != 0
    }

    /// Reset the descriptor for reuse.
    #[inline]
    pub const fn reset(&mut self) {
        self.status = 0;
        self.errors = 0;
        self.length = 0;
    }
}

/// Transmit descriptor for the e1000e NIC.
#[repr(C, packed)]
pub struct TxDescriptor {
    buffer_addr: PhysAddr,
    length: u16,
    cso: u8,
    cmd: u8,
    status: u8,
    css: u8,
    special: u16,
}

impl TxDescriptor {
    /// End of Packet - This is the last descriptor for a packet
    const CMD_EOP: u8 = 1 << 0;
    /// Insert FCS/CRC - Hardware should insert Ethernet CRC
    const CMD_IFCS: u8 = 1 << 1;
    /// Report Status - Hardware should update the status field
    const CMD_RS: u8 = 1 << 3;

    /// Descriptor Done - Hardware has finished transmitting
    const STATUS_DD: u8 = 1 << 0;

    /// Create a new TX descriptor with the given buffer address, marked as done initially.
    #[must_use]
    #[inline]
    pub const fn new(buffer_addr: PhysAddr, length: u16) -> Self {
        Self {
            buffer_addr,
            length,
            cso: 0,
            cmd: 0,
            status: Self::STATUS_DD, // Mark as done initially
            css: 0,
            special: 0,
        }
    }

    /// Check if the hardware has finished transmitting this descriptor.
    #[must_use]
    #[inline]
    pub const fn is_done(&self) -> bool {
        self.status & Self::STATUS_DD != 0
    }

    /// Prepare the descriptor for sending a packet of the given length.
    ///
    /// This sets up the descriptor with:
    /// - EOP (End of Packet)
    /// - IFCS (Insert FCS/CRC)
    /// - RS (Report Status)
    #[inline]
    pub const fn prepare_for_send(&mut self, length: u16) {
        self.length = length;
        self.cmd = Self::CMD_EOP | Self::CMD_IFCS | Self::CMD_RS;
        self.status = 0;
        self.cso = 0;
        self.css = 0;
    }
}
