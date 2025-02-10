use super::{CompletionEntry, CompletionQueue, SubmissionEntry, SubmissionQueue};

pub struct AdminCompletionQueue(CompletionQueue);
pub struct AdminSubmissionQueue(SubmissionQueue);

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
    pub fn new_identify(target: IdentifyTarget, buffer: *mut u8) -> Self {
        let mut entry = SubmissionEntry::zero_with_opcode(Command::Identify as u8);

        let dword10 = match target {
            IdentifyTarget::Controller => 0x01,
            IdentifyTarget::Namespace(nsid) => {
                entry.nsid = nsid;
                0x00
            }
            IdentifyTarget::NamespaceList => 0x02,
        };
        entry.data_ptr[0] = buffer;
        entry.command_specific[0] = dword10;

        Self(entry)
    }
}
