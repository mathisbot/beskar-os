//! Frame Information Structures (FIS) for AHCI
//!
//! FIS structures are used to communicate between the host and the device.

/// Type of FIS (Frame Information Structure)
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum FisType {
    RegisterHostToDevice = 0x27,
    RegisterDeviceToHost = 0x34,
    DmaActivate = 0x39,
    DmaSetup = 0x41,
    Data = 0x46,
    Bist = 0x58,
    PioSetup = 0x5F,
    SetDeviceBits = 0xA1,
}

/// ATA commands used in FIS structures
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum AtaCommand {
    IdentifyDevice = 0xEC,
    ReadSectorEx = 0x24,
    WriteSectorEx = 0x34,
    ReadDmaEx = 0x25,
    WriteDmaEx = 0x35,
}

/// Host-to-Device Register FIS
///
/// This structure is used to send commands from the host to the device.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
pub struct FisH2D {
    pub fis_type: u8,
    /// Bits 0-3: Port multiplier, Bit 7: 1-Command 0-Control
    pub pmport_c: u8,
    pub command: u8,
    pub feature_l: u8,
    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,
    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub feature_h: u8,
    pub count_l: u8,
    pub count_h: u8,
    pub icc: u8,
    pub control: u8,
    _reserved: [u8; 4],
}

impl FisH2D {
    /// Create a new Host-to-Device FIS
    pub const fn new() -> Self {
        Self {
            fis_type: FisType::RegisterHostToDevice as u8,
            pmport_c: 0x80, // Command flag
            command: 0,
            feature_l: 0,
            lba0: 0,
            lba1: 0,
            lba2: 0,
            device: 0,
            lba3: 0,
            lba4: 0,
            lba5: 0,
            feature_h: 0,
            count_l: 0,
            count_h: 0,
            icc: 0,
            control: 0,
            _reserved: [0; 4],
        }
    }

    /// Set LBA (48-bit)
    pub fn set_lba(&mut self, lba: u64) {
        self.lba0 = (lba & 0xFF) as u8;
        self.lba1 = ((lba >> 8) & 0xFF) as u8;
        self.lba2 = ((lba >> 16) & 0xFF) as u8;
        self.lba3 = ((lba >> 24) & 0xFF) as u8;
        self.lba4 = ((lba >> 32) & 0xFF) as u8;
        self.lba5 = ((lba >> 40) & 0xFF) as u8;
    }

    /// Set sector count (16-bit)
    pub fn set_count(&mut self, count: u16) {
        self.count_l = (count & 0xFF) as u8;
        self.count_h = ((count >> 8) & 0xFF) as u8;
    }

    /// Get LBA
    pub fn lba(&self) -> u64 {
        (self.lba0 as u64)
            | ((self.lba1 as u64) << 8)
            | ((self.lba2 as u64) << 16)
            | ((self.lba3 as u64) << 24)
            | ((self.lba4 as u64) << 32)
            | ((self.lba5 as u64) << 40)
    }

    /// Get sector count
    pub fn count(&self) -> u16 {
        (self.count_l as u16) | ((self.count_h as u16) << 8)
    }
}

/// Device-to-Host Register FIS
///
/// This structure is used to receive device status from the device to the host.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
pub struct FisD2H {
    pub fis_type: u8,
    /// Bits 0-3: Port multiplier, Bit 6: Interrupt
    pub pmport_i: u8,
    pub status: u8,
    pub error: u8,
    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,
    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    _reserved1: u8,
    pub count_l: u8,
    pub count_h: u8,
    _reserved2: [u8; 6],
}

impl FisD2H {
    /// Check if this FIS represents an error
    pub fn is_error(&self) -> bool {
        (self.status & 0x01) != 0 // ERR bit
    }

    /// Get the error bits
    pub fn error_bits(&self) -> u8 {
        self.error
    }

    /// Get LBA
    pub fn lba(&self) -> u64 {
        (self.lba0 as u64)
            | ((self.lba1 as u64) << 8)
            | ((self.lba2 as u64) << 16)
            | ((self.lba3 as u64) << 24)
            | ((self.lba4 as u64) << 32)
            | ((self.lba5 as u64) << 40)
    }

    /// Get sector count
    pub fn count(&self) -> u16 {
        (self.count_l as u16) | ((self.count_h as u16) << 8)
    }
}

/// DMA Setup FIS
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
pub struct DmaSetup {
    pub fis_type: u8,
    /// Bits 0-3: Port multiplier, Bit 5: Direction, Bit 6: Interrupt, Bit 7: Auto-activate
    pub pmport: u8,
    _reserved1: [u8; 2],
    pub dma_buffer_id: u64,
    _reserved2: [u8; 4],
    pub dma_buffer_offset: u32,
    pub transfer_count: u32,
    _reserved3: [u8; 4],
}

/// PIO Setup FIS
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
pub struct PioSetup {
    pub fis_type: u8,
    /// Bits 0-3: Port multiplier, Bit 5: Direction, Bit 6: Interrupt
    pub pmport: u8,
    pub status: u8,
    pub error: u8,
    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,
    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    _reserved1: u8,
    pub count_l: u8,
    pub count_h: u8,
    _reserved2: u8,
    pub e_status: u8,
    pub transfer_count: u16,
    _reserved3: [u8; 2],
}
