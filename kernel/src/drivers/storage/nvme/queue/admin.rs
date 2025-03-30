use core::num::NonZeroU8;

use beskar_core::{
    arch::commons::{PhysAddr, paging::Frame},
    drivers::DriverResult,
};
use hyperdrive::ptrs::volatile::{ReadWrite, Volatile};

use super::{CompletionEntry, CompletionQueue, SubmissionEntry, SubmissionQueue};

pub struct AdminCompletionQueue(CompletionQueue);

impl AdminCompletionQueue {
    #[inline]
    pub fn new(doorbell: Volatile<ReadWrite, u16>) -> DriverResult<Self> {
        Ok(Self(CompletionQueue::new(doorbell)?))
    }

    #[must_use]
    #[inline]
    pub const fn paddr(&self) -> PhysAddr {
        self.0.paddr()
    }

    #[inline]
    pub fn pop(&mut self) -> Option<AdminCompletionEntry> {
        self.0.pop().map(AdminCompletionEntry)
    }
}

pub struct AdminSubmissionQueue(SubmissionQueue);

impl AdminSubmissionQueue {
    #[inline]
    pub fn new(doorbell: Volatile<ReadWrite, u16>) -> DriverResult<Self> {
        Ok(Self(SubmissionQueue::new(doorbell)?))
    }

    #[must_use]
    #[inline]
    pub const fn paddr(&self) -> PhysAddr {
        self.0.paddr()
    }

    #[inline]
    pub fn push(&mut self, entry: &AdminSubmissionEntry) {
        self.0.push(entry.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    DeleteIOSubmissionQueue = 0x00,
    CreateIOSubmissionQueue = 0x01,
    GetLogPage = 0x02,
    DeleteIOCompletionQueue = 0x04,
    CreateIOCompletionQueue = 0x05,
    Identify = 0x06,
    Abort = 0x08,
    SetFeatures = 0x09,
    GetFeatures = 0x0A,
    AsynchronousEventRequest = 0x0C,
    NamespaceManagment = 0x0D,
    FirmwareCommit = 0x10,
    FirmwareImageDownload = 0x11,
    DeviceSelfTest = 0x14,
    NamespaceAttachemt = 0x15,
    KeepAlive = 0x18,
    DirectiveSend = 0x19,
    DirectiveReceive = 0x1A,
    VirtualizationManagement = 0x1C,
    NVMeMiSend = 0x1D,
    NVMeMIReceive = 0x1E,
    CapacityManagement = 0x20,
    Lockdown = 0x24,
    DoorbellBufferConfig = 0x7C,
    FabricsCommands = 0x7F,
    FormatNVM = 0x80,
    SecuritySend = 0x81,
    SecurityReceive = 0x82,
    Sanitize = 0x84,
    GetLBAStatus = 0x86,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifyTarget {
    Controller,
    Namespace(u32),
    NamespaceList,
}

pub struct AdminSubmissionEntry(SubmissionEntry);

impl AdminSubmissionEntry {
    pub fn new_identify(target: IdentifyTarget, buffer: Frame) -> Self {
        let mut entry = SubmissionEntry::zero_with_opcode(Command::Identify as u8);

        let dword10 = match target {
            IdentifyTarget::Controller => 0x01,
            IdentifyTarget::Namespace(nsid) => {
                entry.nsid = nsid;
                0x00
            }
            IdentifyTarget::NamespaceList => 0x02,
        };
        entry.data_ptr[0] = buffer.start_address().as_u64() as _;
        entry.command_specific[0] = dword10;

        Self(entry)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct IdentifyController {
    pci_vid: u16,
    pci_ssvid: u16,
    /// Serial number as an ASCII string
    serial_number: [u8; 20],
    /// Model number as an ASCII string
    model_number: [u8; 40],
    /// Firmware revision as an ASCII string
    firmware_revision: [u8; 8],
    /// Recommended arbitration burst
    ///
    /// The value is in commands and is reported as a power of two (2^n)
    rab: u8,
    ieee_oui: [u8; 3],
    /// ## Warning
    ///
    /// Optional field.
    cmpion: u8,
    /// The value is in units of the minimum memory page size (CAP.MPSMIN)
    /// and is reported as a power of two (2^n).
    /// A value of 0h indicates that there is no maximum data transfer size.
    maximum_data_transfer_size: u8,
    controller_id: u16,
    version: u32,
    /// Expected latency in µs to resume from Runtime D3
    rtd3r: u32,
    /// Expected latency in µs to enter Runtime D3
    rtd3e: u32,
    /// This value is a bitflag.
    /// Refer to the specification p.322
    oaes: u32,
    /// This value is a bitflag.
    /// Refer to the specification p.323
    controller_attr: u32,
    /// ## Warning
    ///
    /// Optional field.
    rrls: u16,
    /// Boot partition capable
    bpcap: u8,
}

impl IdentifyController {
    #[must_use]
    #[inline]
    pub const fn pci_vid(&self) -> u16 {
        self.pci_vid
    }

    #[must_use]
    #[inline]
    pub const fn pci_ssvid(&self) -> u16 {
        self.pci_ssvid
    }

    #[must_use]
    #[inline]
    pub const fn serial_number(&self) -> &[u8; 20] {
        &self.serial_number
    }

    #[must_use]
    #[inline]
    pub const fn model_number(&self) -> &[u8; 40] {
        &self.model_number
    }

    #[must_use]
    #[inline]
    pub const fn firmware_revision(&self) -> &[u8; 8] {
        &self.firmware_revision
    }

    #[must_use]
    #[inline]
    pub const fn rab(&self) -> u8 {
        self.rab
    }

    #[must_use]
    #[inline]
    pub const fn ieee_oui(&self) -> &[u8; 3] {
        &self.ieee_oui
    }

    #[must_use]
    #[inline]
    pub const fn cmpion(&self) -> u8 {
        self.cmpion
    }

    #[must_use]
    #[inline]
    /// The value is in units of the minimum memory page size (CAP.MPSMIN)
    /// and is reported as a power of two (2^n).
    pub const fn maximum_data_transfer_size(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.maximum_data_transfer_size)
    }

    #[must_use]
    #[inline]
    pub const fn controller_id(&self) -> u16 {
        self.controller_id
    }

    #[must_use]
    #[inline]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    #[inline]
    /// Expected latency in µs to resume from Runtime D3
    pub const fn rtd3r(&self) -> u32 {
        self.rtd3r
    }

    #[must_use]
    #[inline]
    /// Expected latency in µs to enter Runtime D3
    pub const fn rtd3e(&self) -> u32 {
        self.rtd3e
    }

    #[must_use]
    #[inline]
    /// This value is a bitflag.
    /// Refer to the specification p.322
    pub const fn oaes(&self) -> u32 {
        self.oaes
    }

    #[must_use]
    #[inline]
    /// This value is a bitflag.
    /// Refer to the specification p.323
    pub const fn controller_attr(&self) -> u32 {
        self.controller_attr
    }

    #[must_use]
    #[inline]
    pub const fn rrls(&self) -> u16 {
        self.rrls
    }

    #[must_use]
    #[inline]
    /// Boot partition capable
    pub const fn bpcap(&self) -> u8 {
        self.bpcap
    }
}

pub struct AdminCompletionEntry(CompletionEntry);
