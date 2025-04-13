use core::mem::size_of;

use super::ring::RingElement;

/// Transfer Request Block (TRB)
///
/// TRBs are used for commands, transfers, and events.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
pub struct Trb {
    /// Parameter field (8 bytes)
    pub parameter: u64,
    /// Status field (4 bytes)
    pub status: u32,
    /// Control field (4 bytes)
    pub control: u32,
}

impl Trb {
    /// Size of a TRB in bytes
    pub const SIZE: usize = size_of::<Self>();

    /// Create a new TRB with all fields set to 0
    #[must_use]
    pub const fn new() -> Self {
        Self {
            parameter: 0,
            status: 0,
            control: 0,
        }
    }

    /// Get the TRB type from the control field
    #[must_use]
    pub fn trb_type(&self) -> TrbType {
        let type_val = (self.control >> 10) & 0x3F;
        TrbType::from_u8(type_val as u8).unwrap()
    }

    /// Set the TRB type in the control field
    pub fn set_trb_type(&mut self, trb_type: TrbType) {
        self.control = (self.control & !(0x3F << 10)) | ((trb_type as u32) << 10);
    }

    /// Get the cycle bit from the control field
    #[must_use]
    pub fn cycle_bit(&self) -> bool {
        (self.control & 0x1) != 0
    }

    /// Set the cycle bit in the control field
    pub fn set_cycle_bit(&mut self, cycle: bool) {
        self.control = (self.control & !0x1) | u32::from(cycle);
    }

    /// Toggle the cycle bit in the control field
    pub fn toggle_cycle_bit(&mut self) {
        self.control ^= 0x1;
    }
}

/// TRB Types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TrbType {
    // Reserved = 0,
    Normal = 1,
    SetupStage = 2,
    DataStage = 3,
    StatusStage = 4,
    Isoch = 5,
    Link = 6,
    EventData = 7,
    NoOp = 8,
    EnableSlotCommand = 9,
    DisableSlotCommand = 10,
    AddressDeviceCommand = 11,
    ConfigureEndpointCommand = 12,
    EvaluateContextCommand = 13,
    ResetEndpointCommand = 14,
    StopEndpointCommand = 15,
    SetTRDequeuePointerCommand = 16,
    ResetDeviceCommand = 17,
    // /// Virtualization only
    // ForceEventCommand = 18,
    /// Optional
    NegotiateBandwidthCommand = 19,
    /// Optional
    SetLatencyToleranceValueCommand = 20,
    /// Optional
    GetPortBandwidthCommand = 21,
    ForceHeaderCommand = 22,
    NoOpCommand = 23,
    /// Optional
    GetExtendedPropertyCommand = 24,
    /// Optional
    SetExtendedPropertyCommand = 25,
    // 26 - 31 are reserved
    TransferEvent = 32,
    CommandCompletionEvent = 33,
    PortStatusChangeEvent = 34,
    /// Optional
    BandwidthRequestEvent = 35,
    // /// Virtualization only
    // DoorbellEvent = 36,
    HostControllerEvent = 37,
    DeviceNotificationEvent = 38,
    MfindexWrapEvent = 39,
    // 40 - 47 are reserved
    // 48 - 63 are vendor specific and optional
}

impl TrbType {
    /// Convert a u8 to a TrbType
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            // 0 => Some(Self::Reserved),
            1 => Some(Self::Normal),
            2 => Some(Self::SetupStage),
            3 => Some(Self::DataStage),
            4 => Some(Self::StatusStage),
            5 => Some(Self::Isoch),
            6 => Some(Self::Link),
            7 => Some(Self::EventData),
            8 => Some(Self::NoOp),
            9 => Some(Self::EnableSlotCommand),
            10 => Some(Self::DisableSlotCommand),
            11 => Some(Self::AddressDeviceCommand),
            12 => Some(Self::ConfigureEndpointCommand),
            13 => Some(Self::EvaluateContextCommand),
            14 => Some(Self::ResetEndpointCommand),
            15 => Some(Self::StopEndpointCommand),
            16 => Some(Self::SetTRDequeuePointerCommand),
            17 => Some(Self::ResetDeviceCommand),
            // 18 => Some(Self::ForceEventCommand),
            19 => Some(Self::NegotiateBandwidthCommand),
            20 => Some(Self::SetLatencyToleranceValueCommand),
            21 => Some(Self::GetPortBandwidthCommand),
            22 => Some(Self::ForceHeaderCommand),
            23 => Some(Self::NoOpCommand),
            24 => Some(Self::GetExtendedPropertyCommand),
            25 => Some(Self::SetExtendedPropertyCommand),
            32 => Some(Self::TransferEvent),
            33 => Some(Self::CommandCompletionEvent),
            34 => Some(Self::PortStatusChangeEvent),
            35 => Some(Self::BandwidthRequestEvent),
            // 36 => Some(Self::DoorbellEvent),
            37 => Some(Self::HostControllerEvent),
            38 => Some(Self::DeviceNotificationEvent),
            39 => Some(Self::MfindexWrapEvent),
            _ => None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Completion Code values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompletionCode {
    Invalid = 0,
    Success = 1,
    DataBufferError = 2,
    BabbleDetectedError = 3,
    UsbTransactionError = 4,
    TrbError = 5,
    StallError = 6,
    ResourceError = 7,
    BandwidthError = 8,
    NoSlotsAvailableError = 9,
    InvalidStreamTypeError = 10,
    SlotNotEnabledError = 11,
    EndpointNotEnabledError = 12,
    ShortPacket = 13,
    RingUnderrun = 14,
    RingOverrun = 15,
    VfEventRingFullError = 16,
    ParameterError = 17,
    BandwidthOverrunError = 18,
    ContextStateError = 19,
    NoPingResponseError = 20,
    EventRingFullError = 21,
    IncompatibleDeviceError = 22,
    MissedServiceError = 23,
    CommandRingStopped = 24,
    CommandAborted = 25,
    Stopped = 26,
    StoppedLengthInvalid = 27,
    StoppedShortPacket = 28,
    MaxExitLatencyTooLargeError = 29,
    // 30 is reserved
    IsochBufferOverrun = 31,
    EventLostError = 32,
    UndefinedError = 33,
    InvalidStreamIdError = 34,
    SecondaryBandwidthError = 35,
    SplitTransactionError = 36,
    // 37-191 are reserved
    // 192-223 are vendor specific errors
    // 224-255 are vendor specific info
}

impl CompletionCode {
    /// Convert a u8 to a CompletionCode
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Invalid),
            1 => Some(Self::Success),
            2 => Some(Self::DataBufferError),
            3 => Some(Self::BabbleDetectedError),
            4 => Some(Self::UsbTransactionError),
            5 => Some(Self::TrbError),
            6 => Some(Self::StallError),
            7 => Some(Self::ResourceError),
            8 => Some(Self::BandwidthError),
            9 => Some(Self::NoSlotsAvailableError),
            10 => Some(Self::InvalidStreamTypeError),
            11 => Some(Self::SlotNotEnabledError),
            12 => Some(Self::EndpointNotEnabledError),
            13 => Some(Self::ShortPacket),
            14 => Some(Self::RingUnderrun),
            15 => Some(Self::RingOverrun),
            16 => Some(Self::VfEventRingFullError),
            17 => Some(Self::ParameterError),
            18 => Some(Self::BandwidthOverrunError),
            19 => Some(Self::ContextStateError),
            20 => Some(Self::NoPingResponseError),
            21 => Some(Self::EventRingFullError),
            22 => Some(Self::IncompatibleDeviceError),
            23 => Some(Self::MissedServiceError),
            24 => Some(Self::CommandRingStopped),
            25 => Some(Self::CommandAborted),
            26 => Some(Self::Stopped),
            27 => Some(Self::StoppedLengthInvalid),
            28 => Some(Self::StoppedShortPacket),
            29 => Some(Self::MaxExitLatencyTooLargeError),
            31 => Some(Self::IsochBufferOverrun),
            32 => Some(Self::EventLostError),
            33 => Some(Self::UndefinedError),
            34 => Some(Self::InvalidStreamIdError),
            35 => Some(Self::SecondaryBandwidthError),
            36 => Some(Self::SplitTransactionError),
            _ => None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Command TRB - Enable Slot Command
#[derive(Debug, Clone, Copy)]
pub struct EnableSlotCommandTrb {
    trb: Trb,
}

impl EnableSlotCommandTrb {
    /// Create a new Enable Slot Command TRB
    #[must_use]
    pub fn new(slot_type: u8) -> Self {
        let mut trb = Trb::new();
        trb.set_trb_type(TrbType::EnableSlotCommand);
        trb.control |= (u32::from(slot_type) & 0x1F) << 16;
        Self { trb }
    }

    /// Convert to a generic TRB
    #[must_use]
    pub const fn as_trb(&self) -> &Trb {
        &self.trb
    }

    /// Convert to a mutable generic TRB
    pub fn as_trb_mut(&mut self) -> &mut Trb {
        &mut self.trb
    }
}

/// Command TRB - Address Device Command
#[derive(Debug, Clone, Copy)]
pub struct AddressDeviceCommandTrb {
    trb: Trb,
}

impl AddressDeviceCommandTrb {
    /// Create a new Address Device Command TRB
    #[must_use]
    pub fn new(input_context_ptr: u64, slot_id: u8) -> Self {
        let mut trb = Trb::new();
        trb.parameter = input_context_ptr;
        trb.set_trb_type(TrbType::AddressDeviceCommand);
        trb.control |= (u32::from(slot_id) & 0xFF) << 24;
        Self { trb }
    }

    /// Convert to a generic TRB
    #[must_use]
    pub const fn as_trb(&self) -> &Trb {
        &self.trb
    }

    /// Convert to a mutable generic TRB
    pub fn as_trb_mut(&mut self) -> &mut Trb {
        &mut self.trb
    }
}

/// Command TRB - Configure Endpoint Command
#[derive(Debug, Clone, Copy)]
pub struct ConfigureEndpointCommandTrb {
    trb: Trb,
}

impl ConfigureEndpointCommandTrb {
    /// Create a new Configure Endpoint Command TRB
    #[must_use]
    pub fn new(input_context_ptr: u64, slot_id: u8, deconfigure: bool) -> Self {
        let mut trb = Trb::new();
        trb.parameter = input_context_ptr;
        trb.set_trb_type(TrbType::ConfigureEndpointCommand);
        if deconfigure {
            trb.control |= 1 << 9;
        }
        trb.control |= (u32::from(slot_id) & 0xFF) << 24;
        Self { trb }
    }

    /// Convert to a generic TRB
    #[must_use]
    pub const fn as_trb(&self) -> &Trb {
        &self.trb
    }

    /// Convert to a mutable generic TRB
    pub fn as_trb_mut(&mut self) -> &mut Trb {
        &mut self.trb
    }
}

/// Link TRB - Used to link to another segment in a ring
#[derive(Debug, Clone, Copy)]
pub struct LinkTrb {
    trb: Trb,
}

impl LinkTrb {
    /// Create a new Link TRB
    #[must_use]
    pub fn new(ring_segment_ptr: u64, toggle_cycle: bool) -> Self {
        let mut trb = Trb::new();
        trb.parameter = ring_segment_ptr;
        trb.set_trb_type(TrbType::Link);
        if toggle_cycle {
            trb.control |= 1 << 1;
        }
        Self { trb }
    }

    /// Convert to a generic TRB
    #[must_use]
    pub const fn as_trb(&self) -> &Trb {
        &self.trb
    }

    /// Convert to a mutable generic TRB
    pub fn as_trb_mut(&mut self) -> &mut Trb {
        &mut self.trb
    }
}

/// Event TRB - Command Completion Event
#[derive(Debug, Clone, Copy)]
pub struct CommandCompletionEventTrb {
    trb: Trb,
}

impl CommandCompletionEventTrb {
    /// Create a new Command Completion Event TRB
    #[must_use]
    pub fn new(command_trb_ptr: u64, completion_code: CompletionCode, slot_id: u8) -> Self {
        let mut trb = Trb::new();
        trb.parameter = command_trb_ptr;
        trb.status = (u32::from(completion_code as u8) << 24) | (u32::from(slot_id) << 16);
        trb.set_trb_type(TrbType::CommandCompletionEvent);
        Self { trb }
    }

    /// Get the command TRB pointer
    #[must_use]
    pub const fn command_trb_ptr(&self) -> u64 {
        self.trb.parameter
    }

    /// Get the completion code
    #[must_use]
    pub fn completion_code(&self) -> Option<CompletionCode> {
        CompletionCode::from_u8(u8::try_from(self.trb.status >> 24).unwrap())
    }

    /// Get the slot ID
    #[must_use]
    pub fn slot_id(&self) -> u8 {
        u8::try_from(self.trb.control >> 24).unwrap()
    }

    /// Convert to a generic TRB
    #[must_use]
    pub const fn as_trb(&self) -> &Trb {
        &self.trb
    }

    /// Convert from a generic TRB
    #[must_use]
    pub fn from_trb(trb: &Trb) -> Self {
        Self { trb: *trb }
    }
}

/// Event TRB - Port Status Change Event
#[derive(Debug, Clone, Copy)]
pub struct PortStatusChangeEventTrb {
    trb: Trb,
}

impl PortStatusChangeEventTrb {
    /// Create a new Port Status Change Event TRB
    #[must_use]
    pub fn new(port_id: u8) -> Self {
        let mut trb = Trb::new();
        trb.parameter = 0;
        trb.status = 0;
        trb.set_trb_type(TrbType::PortStatusChangeEvent);
        trb.control |= (u32::from(port_id) & 0xFF) << 24;
        Self { trb }
    }

    /// Convert to a generic TRB
    #[must_use]
    pub const fn as_trb(&self) -> &Trb {
        &self.trb
    }

    /// Convert from a generic TRB
    #[must_use]
    pub fn from_trb(trb: &Trb) -> Self {
        Self { trb: *trb }
    }
}

/// Transfer TRB - Setup Stage
#[derive(Debug, Clone, Copy)]
pub struct SetupStageTrb {
    trb: Trb,
}

impl SetupStageTrb {
    /// Create a new Setup Stage TRB
    #[must_use]
    pub fn new(
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        length: u16,
        transfer_type: u8,
    ) -> Self {
        let mut trb = Trb::new();
        trb.parameter = u64::from(request_type)
            | (u64::from(request) << 8)
            | (u64::from(value) << 16)
            | (u64::from(index) << 32)
            | (u64::from(length) << 48);
        trb.set_trb_type(TrbType::SetupStage);
        trb.control |= (u32::from(transfer_type) & 0x3) << 16;
        trb.control |= 1 << 6; // Immediate Data
        Self { trb }
    }

    /// Convert to a generic TRB
    #[must_use]
    pub const fn as_trb(&self) -> &Trb {
        &self.trb
    }

    /// Convert to a mutable generic TRB
    pub fn as_trb_mut(&mut self) -> &mut Trb {
        &mut self.trb
    }
}

/// Transfer TRB - Data Stage
#[derive(Debug, Clone, Copy)]
pub struct DataStageTrb {
    trb: Trb,
}

impl DataStageTrb {
    /// Create a new Data Stage TRB
    #[must_use]
    pub fn new(
        data_buffer_ptr: u64,
        transfer_length: u32,
        direction_in: bool,
        interrupt_on_completion: bool,
    ) -> Self {
        let mut trb = Trb::new();
        trb.parameter = data_buffer_ptr;
        trb.status = transfer_length;
        trb.set_trb_type(TrbType::DataStage);
        if direction_in {
            trb.control |= 1 << 16;
        }
        if interrupt_on_completion {
            trb.control |= 1 << 5;
        }
        Self { trb }
    }

    /// Convert to a generic TRB
    #[must_use]
    pub const fn as_trb(&self) -> &Trb {
        &self.trb
    }

    /// Convert to a mutable generic TRB
    pub fn as_trb_mut(&mut self) -> &mut Trb {
        &mut self.trb
    }
}

/// Transfer TRB - Status Stage
#[derive(Debug, Clone, Copy)]
pub struct StatusStageTrb {
    trb: Trb,
}

impl StatusStageTrb {
    /// Create a new Status Stage TRB
    #[must_use]
    pub fn new(direction_in: bool, interrupt_on_completion: bool) -> Self {
        let mut trb = Trb::new();
        trb.set_trb_type(TrbType::StatusStage);
        if direction_in {
            trb.control |= 1 << 16;
        }
        if interrupt_on_completion {
            trb.control |= 1 << 5;
        }
        Self { trb }
    }

    /// Convert to a generic TRB
    #[must_use]
    pub const fn as_trb(&self) -> &Trb {
        &self.trb
    }

    /// Convert to a mutable generic TRB
    pub fn as_trb_mut(&mut self) -> &mut Trb {
        &mut self.trb
    }
}

impl RingElement for Trb {
    #[inline]
    fn set_cycle_bit(&mut self, cycle_bit: bool) {
        self.set_cycle_bit(cycle_bit);
    }
}
