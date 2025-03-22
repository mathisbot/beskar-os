use core::ptr::NonNull;

use beskar_core::arch::commons::VirtAddr;
use hyperdrive::ptrs::volatile::{ReadOnly, Volatile};

#[derive(Clone, Copy)]
pub struct CapabilitiesRegisters {
    base: Volatile<ReadOnly, u32>,
}

impl CapabilitiesRegisters {
    pub const MIN_LENGTH: usize = 0x20;

    const CAP_LENGTH: usize = 0x00;
    const HCI_VERSION: usize = 0x02;
    const HCS_PARAMS1: usize = 0x04;
    const HCS_PARAMS2: usize = 0x08;
    const HCS_PARAMS3: usize = 0x0C;
    const HCC_PARAMS1: usize = 0x10;
    const DBOFF: usize = 0x14;
    const RTSOFF: usize = 0x18;
    const HCC_PARAMS2: usize = 0x1C;

    #[must_use]
    pub const fn new(base: VirtAddr) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap());
        Self { base }
    }

    #[must_use]
    /// Offset of the first operational register from the base address
    pub fn cap_length(self) -> u8 {
        unsafe { self.base.cast::<u8>().byte_add(Self::CAP_LENGTH).read() }
    }

    #[must_use]
    pub fn hci_version(self) -> HciVersion {
        // unsafe { self.base.cast::<u16>().byte_add(Self::HCI_VERSION).read() }
        // There currently is a bug in QEMU, where xHCI registers do not support DWORD reads.
        // This is a workaround to read the register as a QWORD and extract the bytes.
        //
        // According to the xHCI specification, these fields should allow 1-4 bytes reads,
        // so it is safe to do so even on real hardware.
        let qword = unsafe { self.base.read() };
        let [_cap_len, _reserved, minor, major] = qword.to_le_bytes();

        HciVersion { major, minor }
    }

    #[must_use]
    pub fn hcs_params1(self) -> HcsParams1 {
        HcsParams1::new(unsafe { self.base.byte_add(Self::HCS_PARAMS1).read() })
    }

    #[must_use]
    pub fn hcs_params2(self) -> HcsParams2 {
        HcsParams2::new(unsafe { self.base.byte_add(Self::HCS_PARAMS2).read() })
    }

    #[must_use]
    pub fn hcs_params3(self) -> HcsParams3 {
        HcsParams3::new(unsafe { self.base.byte_add(Self::HCS_PARAMS3).read() })
    }

    #[must_use]
    pub fn hcc_params1(self) -> HccParams1 {
        HccParams1::new(unsafe { self.base.byte_add(Self::HCC_PARAMS1).read() })
    }

    #[must_use]
    /// Doorbell array offset in bytes.
    pub fn dboff(self) -> u32 {
        unsafe { self.base.byte_add(Self::DBOFF).read() & !0b11 }
    }

    #[must_use]
    /// Runtime register space offset in bytes.
    pub fn rtsoff(self) -> u32 {
        unsafe { self.base.byte_add(Self::RTSOFF).read() & !0b1_1111 }
    }

    #[must_use]
    pub fn hcc_params2(self) -> u32 {
        unsafe { self.base.byte_add(Self::HCC_PARAMS2).read() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct HciVersion {
    major: u8,
    minor: u8,
}

impl core::fmt::Display for HciVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "HCI {}.{}", self.major, self.minor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct HcsParams1 {
    /// Maximum number of Device Context Structures and
    /// Doorbell Array entries this host controller can support.
    /// Value 0 is reserved.
    max_slots: u8,
    /// Number of Interrupters implemented on this host controller.
    /// Each Interrupter may be allocated to a MSI or MSI-X vector
    /// and controls its generation and moderation.
    /// Value 0 is undefined.
    max_intrs: u16,
    /// Maximum Port Number value:
    /// highest numbered Port Register Set that are addressable in the
    /// Operational Register Space
    max_ports: u8,
}

impl HcsParams1 {
    #[must_use]
    pub fn new(value: u32) -> Self {
        let max_slots = u8::try_from(value & 0xFF).unwrap();
        let max_intrs = u16::try_from((value >> 8) & 0x7FF).unwrap();
        let max_ports = u8::try_from((value >> 24) & 0xFF).unwrap();

        Self {
            max_slots,
            max_intrs,
            max_ports,
        }
    }

    #[must_use]
    #[inline]
    pub const fn max_slots(self) -> u8 {
        self.max_slots
    }

    #[must_use]
    #[inline]
    pub const fn max_intrs(self) -> u16 {
        self.max_intrs
    }

    #[must_use]
    #[inline]
    pub const fn max_ports(self) -> u8 {
        self.max_ports
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HcsParams2 {
    /// Minimum distance (in time, specified in number of (micro)frames)
    /// that it is required to stay ahead of the host controller while adding TRBs,
    /// in order to have the host controller process them at the correct time.
    ist: u8,
    /// Maximum value for the Event Ring Segment Table Base Size registers.
    erst_max: u8,
    /// Number of Scratchpad Buffers that shall be reserved for the xHC.
    max_scratchpad_bufs: u16,
    /// Wether the xHC supports Scratchpad Buffer Restore.
    scratchpad_restore: bool,
}

impl HcsParams2 {
    #[must_use]
    pub fn new(value: u32) -> Self {
        let ist = u8::try_from(value & 0xF).unwrap();
        let erst_max = u8::try_from((value >> 4) & 0xF).unwrap();
        let max_scratchpad_bufs = u16::try_from((value >> 16) & 0x1F).unwrap()
            | u16::try_from((value >> 27) & 0x1F).unwrap();
        let scratchpad_restore = (value & (1 << 26)) != 0;

        Self {
            ist,
            erst_max,
            max_scratchpad_bufs,
            scratchpad_restore,
        }
    }

    #[must_use]
    #[inline]
    pub const fn ist(self) -> u8 {
        self.ist
    }

    #[must_use]
    #[inline]
    pub const fn erst_max(self) -> u8 {
        self.erst_max
    }

    #[must_use]
    #[inline]
    pub const fn max_scratchpad_bufs(self) -> u16 {
        self.max_scratchpad_bufs
    }

    #[must_use]
    #[inline]
    pub const fn scratchpad_restore(self) -> bool {
        self.scratchpad_restore
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HcsParams3 {
    /// Worst case latendy to transition a root hub PLS
    /// from U1 to U0.
    u1_device_exit_latency: u8,
    /// Worst case latendy to transition a root hub PLS
    /// from U2 to U0.
    u2_device_exit_latency: u16,
}

impl HcsParams3 {
    #[must_use]
    pub fn new(value: u32) -> Self {
        let u1_device_exit_latency = u8::try_from(value & 0xFF).unwrap();
        let u2_device_exit_latency = u16::try_from((value >> 16) & 0xFFFF).unwrap();

        Self {
            u1_device_exit_latency,
            u2_device_exit_latency,
        }
    }

    #[must_use]
    #[inline]
    pub const fn u1_device_exit_latency(self) -> u8 {
        self.u1_device_exit_latency
    }

    #[must_use]
    #[inline]
    pub const fn u2_device_exit_latency(self) -> u16 {
        self.u2_device_exit_latency
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HccParams1 {
    value: u32,
}

impl HccParams1 {
    const AC64: i32 = 0;
    const BNC: i32 = 1;
    const CSZ: i32 = 2;
    const PPC: i32 = 3;
    const PIND: i32 = 4;
    const LHRC: i32 = 5;
    const LTC: i32 = 6;
    const NSS: i32 = 7;
    const PAE: i32 = 8;
    const SPC: i32 = 9;
    const SEC: i32 = 10;
    const CFC: i32 = 11;

    #[must_use]
    #[inline]
    pub const fn new(value: u32) -> Self {
        Self { value }
    }

    #[must_use]
    #[inline]
    /// Whether the xHC has implemented the high order 32
    /// bits of 64 bit register and data structure pointer fields
    pub const fn ac64(self) -> bool {
        (self.value & (1 << Self::AC64)) != 0
    }

    #[must_use]
    #[inline]
    /// Wether the xHC has implemented the
    /// Bandwidth Negotiation
    pub const fn bnc(self) -> bool {
        (self.value & (1 << Self::BNC)) != 0
    }

    #[must_use]
    #[inline]
    /// Wether xHC uses 64 byte Context data structures
    pub const fn csz(self) -> bool {
        (self.value & (1 << Self::CSZ)) != 0
    }

    #[must_use]
    #[inline]
    /// Whether the xHC implementation includes
    /// port power control
    pub const fn ppc(self) -> bool {
        (self.value & (1 << Self::PPC)) != 0
    }

    #[must_use]
    #[inline]
    /// Whether the xHC root hub ports support port indicator control.
    pub const fn pind(self) -> bool {
        (self.value & (1 << Self::PIND)) != 0
    }

    #[must_use]
    #[inline]
    /// Whether the host controller implementation
    /// supports a Light Host Controller Reset.
    pub const fn lhrc(self) -> bool {
        (self.value & (1 << Self::LHRC)) != 0
    }

    #[must_use]
    #[inline]
    /// Whether the xHC supports Latency Tolerance Messaging.
    pub const fn ltc(self) -> bool {
        (self.value & (1 << Self::LTC)) != 0
    }

    #[must_use]
    #[inline]
    /// Wether the xHC supports secondary Stream IDs.
    pub const fn nss(self) -> bool {
        (self.value & (1 << Self::NSS)) != 0
    }

    #[must_use]
    #[inline]
    /// Whether the host controller implementation Parses all Event Data TRBs
    /// while advancing to the next TD after a Short Packet, or it skips all
    /// but the first Event Data TRB.
    pub const fn pae(self) -> bool {
        (self.value & (1 << Self::PAE)) != 0
    }

    #[must_use]
    #[inline]
    /// Whether the host controller implementation is capable of generating
    /// a Stopped - Short Packet Completion Code
    pub const fn spc(self) -> bool {
        (self.value & (1 << Self::SPC)) != 0
    }

    #[must_use]
    #[inline]
    /// Wether the host controller implementation Stream
    /// Context support a Stopped EDTLA field.
    pub const fn sec(self) -> bool {
        (self.value & (1 << Self::SEC)) != 0
    }

    #[must_use]
    #[inline]
    /// Wether the host controller implementation is capable of
    /// matching the Frame ID of consecutive Isoch TDs.
    pub const fn cfc(self) -> bool {
        (self.value & (1 << Self::CFC)) != 0
    }

    #[must_use]
    /// Maximum size Primary Stream Array that the xHC supports.
    pub const fn max_psa_size(self) -> u16 {
        let raw_value = (self.value >> 12) & 0xF;
        2_u16.pow(raw_value + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HccParams2 {
    value: u32,
}

impl HccParams2 {
    const U3C: i32 = 0;
    // TODO: Add the rest of the fields

    #[must_use]
    #[inline]
    pub const fn new(value: u32) -> Self {
        Self { value }
    }

    #[must_use]
    #[inline]
    /// Whether the xHC Root Hub ports support
    /// port Suspend Complete notification.
    pub const fn u3c(self) -> bool {
        (self.value & (1 << Self::U3C)) != 0
    }
}
