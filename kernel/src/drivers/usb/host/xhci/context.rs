use beskar_core::arch::PhysAddr;

/// Device Context Base Address Array (DCBAA)
///
/// An array of pointers to Device Context structures.
#[derive(Debug)]
pub struct DeviceContextBaseAddressArray {
    /// Array of pointers to Device Context structures
    entries: &'static mut [u64],
    /// Number of entries in the array
    max_slots: usize,
}

impl DeviceContextBaseAddressArray {
    /// Create a new Device Context Base Address Array
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `entries` points to a valid array of `max_slots + 1` u64 values
    /// - The memory is properly aligned (64-byte alignment)
    /// - The memory is accessible by the xHCI controller
    #[must_use]
    #[inline]
    pub unsafe fn new(entries: &'static mut [u64], max_slots: usize) -> Self {
        assert!(entries.len() >= max_slots, "DCBAA too small");
        // Zero out all entries
        entries.fill(0);
        Self { entries, max_slots }
    }

    #[inline]
    /// Set a device context entry
    ///
    /// # Panics
    ///
    /// Panics if `slot_id` is 0 or greater than `max_slots`
    pub const fn set_device_context(&mut self, slot_id: usize, context_addr: PhysAddr) {
        assert!(slot_id > 0 && slot_id <= self.max_slots, "Invalid slot ID");
        self.entries[slot_id] = context_addr.as_u64();
    }

    /// Get a device context entry
    ///
    /// # Panics
    ///
    /// Panics if `slot_id` is 0 or greater than `max_slots`
    #[must_use]
    pub const fn get_device_context(&self, slot_id: usize) -> Option<PhysAddr> {
        assert!(slot_id > 0 && slot_id <= self.max_slots, "Invalid slot ID");
        let addr = self.entries[slot_id];
        if addr == 0 {
            None
        } else {
            Some(PhysAddr::new(addr))
        }
    }

    #[inline]
    /// Set the scratchpad buffer array entry (entry 0)
    pub const fn set_scratchpad_buffer_array(&mut self, scratchpad_addr: PhysAddr) {
        self.entries[0] = scratchpad_addr.as_u64();
    }
}

/// Endpoint Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EndpointType {
    /// Not valid
    NotValid = 0,
    /// Isoch Out
    IsochOut = 1,
    /// Bulk Out
    BulkOut = 2,
    /// Interrupt Out
    InterruptOut = 3,
    /// Control Bidirectional
    Control = 4,
    /// Isoch In
    IsochIn = 5,
    /// Bulk In
    BulkIn = 6,
    /// Interrupt In
    InterruptIn = 7,
}

/// Slot Context
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct SlotContext {
    /// DWord 0
    pub field0: u32,
    /// DWord 1
    pub field1: u32,
    /// DWord 2
    pub field2: u32,
    /// DWord 3
    pub field3: u32,
    /// DWord 4-7 (reserved)
    pub reserved: [u32; 4],
}

impl SlotContext {
    #[must_use]
    #[inline]
    /// Create a new Slot Context with all fields set to 0
    pub const fn new() -> Self {
        Self {
            field0: 0,
            field1: 0,
            field2: 0,
            field3: 0,
            reserved: [0; 4],
        }
    }

    #[inline]
    /// Set the route string
    pub const fn set_route_string(&mut self, route_string: u32) {
        self.field0 = (self.field0 & !0xFFFFF) | (route_string & 0xFFFFF);
    }

    #[inline]
    /// Set the speed
    pub fn set_speed(&mut self, speed: u8) {
        self.field0 = (self.field0 & !(0xF << 20)) | (u32::from(speed & 0xF) << 20);
    }

    #[inline]
    /// Set the context entries
    pub fn set_context_entries(&mut self, entries: u8) {
        self.field0 = (self.field0 & !(0x1F << 27)) | (u32::from(entries & 0x1F) << 27);
    }

    #[inline]
    /// Set the root hub port number
    pub fn set_root_hub_port_number(&mut self, port: u8) {
        self.field1 = (self.field1 & !0xFF) | u32::from(port);
    }

    #[inline]
    /// Set the max exit latency
    pub fn set_max_exit_latency(&mut self, latency: u16) {
        self.field1 = (self.field1 & !(0xFFFF << 16)) | (u32::from(latency) << 16);
    }

    #[inline]
    /// Set the interrupter target
    pub fn set_interrupter_target(&mut self, target: u16) {
        self.field2 = (self.field2 & !0x3FF) | u32::from(target & 0x3FF);
    }

    #[inline]
    /// Set the USB device address
    pub fn set_usb_device_address(&mut self, address: u8) {
        self.field2 = (self.field2 & !(0xFF << 24)) | (u32::from(address) << 24);
    }

    #[inline]
    /// Set the slot state
    pub fn set_slot_state(&mut self, state: u8) {
        self.field3 = (self.field3 & !(0x1F << 27)) | (u32::from(state & 0x1F) << 27);
    }
}

/// Endpoint Context
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct EndpointContext {
    /// DWord 0
    pub field0: u32,
    /// DWord 1
    pub field1: u32,
    /// DWord 2 (TR Dequeue Pointer Lo)
    pub tr_dequeue_pointer_lo: u32,
    /// DWord 3 (TR Dequeue Pointer Hi)
    pub tr_dequeue_pointer_hi: u32,
    /// DWord 4
    pub field4: u32,
    /// DWord 5
    pub field5: u32,
    _reserved: [u32; 2],
}

impl EndpointContext {
    #[must_use]
    #[inline]
    /// Create a new Endpoint Context with all fields set to 0
    pub const fn new() -> Self {
        Self {
            field0: 0,
            field1: 0,
            tr_dequeue_pointer_lo: 0,
            tr_dequeue_pointer_hi: 0,
            field4: 0,
            field5: 0,
            _reserved: [0; 2],
        }
    }

    #[inline]
    /// Set the endpoint state
    pub fn set_endpoint_state(&mut self, state: u8) {
        self.field0 = (self.field0 & !0x7) | u32::from(state & 0x7);
    }

    #[inline]
    /// Set the mult value
    pub fn set_mult(&mut self, mult: u8) {
        self.field0 = (self.field0 & !(0x3 << 8)) | (u32::from(mult & 0x3) << 8);
    }

    #[inline]
    /// Set the max primary streams
    pub fn set_max_primary_streams(&mut self, streams: u8) {
        self.field0 = (self.field0 & !(0x1F << 10)) | (u32::from(streams & 0x1F) << 10);
    }

    #[inline]
    /// Set the interval
    pub fn set_interval(&mut self, interval: u8) {
        self.field0 = (self.field0 & !(0xFF << 16)) | (u32::from(interval) << 16);
    }

    #[inline]
    /// Set the error count
    pub fn set_error_count(&mut self, count: u8) {
        self.field1 = (self.field1 & !(0x3 << 1)) | (u32::from(count & 0x3) << 1);
    }

    #[inline]
    /// Set the endpoint type
    pub const fn set_endpoint_type(&mut self, ep_type: EndpointType) {
        self.field1 = (self.field1 & !(0x7 << 3)) | ((ep_type as u32) << 3);
    }

    #[inline]
    /// Set the max burst size
    pub fn set_max_burst_size(&mut self, size: u8) {
        self.field1 = (self.field1 & !(0xFF << 8)) | (u32::from(size) << 8);
    }

    #[inline]
    /// Set the max packet size
    pub fn set_max_packet_size(&mut self, size: u16) {
        self.field1 = (self.field1 & !(0xFFFF << 16)) | (u32::from(size) << 16);
    }

    /// Set the TR Dequeue Pointer
    pub fn set_tr_dequeue_pointer(&mut self, addr: PhysAddr, dcs: bool) {
        let ptr = addr.as_u64() & !0xF;
        let dcs_bit = u32::from(dcs);
        self.tr_dequeue_pointer_lo = u32::try_from(ptr & 0xFFFF_FFFF).unwrap() | dcs_bit;
        self.tr_dequeue_pointer_hi = u32::try_from(ptr >> 32).unwrap();
    }

    #[must_use]
    /// Get the TR Dequeue Pointer
    pub fn tr_dequeue_pointer(&self) -> (PhysAddr, bool) {
        let lo = u64::from(self.tr_dequeue_pointer_lo);
        let hi = u64::from(self.tr_dequeue_pointer_hi) << 32;
        let ptr = (hi | lo) & !0xF;
        let dcs = (lo & 0x1) != 0;
        (PhysAddr::new(ptr), dcs)
    }

    #[inline]
    /// Set the average TRB length
    pub fn set_average_trb_length(&mut self, length: u16) {
        self.field4 = (self.field4 & !0xFFFF) | u32::from(length);
    }
}

/// Input Control Context
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct InputControlContext {
    /// Drop Context Flags
    pub drop_context_flags: u32,
    /// Add Context Flags
    pub add_context_flags: u32,
    _reserved: [u32; 6],
}

impl InputControlContext {
    #[must_use]
    #[inline]
    /// Create a new Input Control Context with all fields set to 0
    pub const fn new() -> Self {
        Self {
            drop_context_flags: 0,
            add_context_flags: 0,
            _reserved: [0; 6],
        }
    }

    #[inline]
    /// Set the drop context flag for a specific context
    pub const fn set_drop_context_flag(&mut self, context_index: u8) {
        self.drop_context_flags |= 1 << context_index;
    }

    #[inline]
    /// Set the add context flag for a specific context
    pub const fn set_add_context_flag(&mut self, context_index: u8) {
        self.add_context_flags |= 1 << context_index;
    }
}

/// Device Context
///
/// A structure containing a Slot Context and 31 Endpoint Contexts.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct DeviceContext {
    /// Slot Context
    pub slot_context: SlotContext,
    /// Endpoint Contexts
    pub endpoint_contexts: [EndpointContext; 31],
}

impl DeviceContext {
    #[must_use]
    #[inline]
    /// Create a new Device Context with all fields set to 0
    pub const fn new() -> Self {
        Self {
            slot_context: SlotContext::new(),
            endpoint_contexts: [EndpointContext::new(); 31],
        }
    }
}

/// Input Context
///
/// A structure containing an Input Control Context, a Slot Context, and 31 Endpoint Contexts.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct InputContext {
    /// Input Control Context
    pub input_control_context: InputControlContext,
    /// Slot Context
    pub slot_context: SlotContext,
    /// Endpoint Contexts
    pub endpoint_contexts: [EndpointContext; 31],
}

impl InputContext {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            input_control_context: InputControlContext::new(),
            slot_context: SlotContext::new(),
            endpoint_contexts: [EndpointContext::new(); 31],
        }
    }
}
