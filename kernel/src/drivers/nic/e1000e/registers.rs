//! E1000e register offsets and bit flags.

/// Register offsets for the e1000e NIC.
pub struct Registers;

#[allow(dead_code)]
impl Registers {
    // Control registers
    pub const CTRL: usize = 0x00000;
    pub const STATUS: usize = 0x00008;
    pub const EEC: usize = 0x00010;
    pub const EERD: usize = 0x00014;
    pub const CTRLEXT: usize = 0x00018;

    // Interrupt registers
    pub const ICR: usize = 0x000C0; // Interrupt Cause Read
    pub const ICS: usize = 0x000C8; // Interrupt Cause Set
    pub const IMS: usize = 0x000D0; // Interrupt Mask Set
    pub const IMC: usize = 0x000D8; // Interrupt Mask Clear

    // Receive registers
    pub const RCTL: usize = 0x00100; // Receive Control
    pub const RDBAL0: usize = 0x02800; // RX Descriptor Base Address Low
    pub const RDBAH0: usize = 0x02804; // RX Descriptor Base Address High
    pub const RDLEN: usize = 0x02808; // RX Descriptor Length
    pub const RDH: usize = 0x02810; // RX Descriptor Head
    pub const RDT: usize = 0x02818; // RX Descriptor Tail

    // Transmit registers
    pub const TCTL: usize = 0x00400; // Transmit Control
    pub const TIPG: usize = 0x00410; // Transmit IPG
    pub const TDBAL0: usize = 0x03800; // TX Descriptor Base Address Low
    pub const TDBAH0: usize = 0x03804; // TX Descriptor Base Address High
    pub const TDLEN: usize = 0x03808; // TX Descriptor Length
    pub const TDH: usize = 0x03810; // TX Descriptor Head
    pub const TDT: usize = 0x03818; // TX Descriptor Tail

    // MAC address registers
    pub const RAL0: usize = 0x05400; // Receive Address Low
    pub const RAH0: usize = 0x05404; // Receive Address High
}

/// RCTL (Receive Control) register flags
pub struct RctlFlags;

#[allow(dead_code)]
impl RctlFlags {
    /// Receiver Enable
    pub const EN: u32 = 1 << 1;
    /// Store Bad Packets
    pub const SBP: u32 = 1 << 2;
    /// Unicast Promiscuous Mode
    pub const UPE: u32 = 1 << 3;
    /// Multicast Promiscuous Mode
    pub const MPE: u32 = 1 << 4;
    /// Long Packet Enable
    pub const LPE: u32 = 1 << 5;

    // Loopback modes
    /// Normal Operation (no loopback)
    pub const LBM_PHY: u32 = 0b00 << 6;
    /// MAC Loopback (for testing)
    pub const LBM_MAC: u32 = 0b10 << 6;

    // Receive Descriptor Minimum Threshold Size
    pub const RDMTS_HALF: u32 = 0b00 << 8;
    pub const RDMTS_QUARTER: u32 = 0b01 << 8;
    pub const RDMTS_EIGHTH: u32 = 0b10 << 8;

    // Multicast Offset
    pub const MO_36: u32 = 0b00 << 12;
    pub const MO_35: u32 = 0b01 << 12;
    pub const MO_34: u32 = 0b10 << 12;
    pub const MO_32: u32 = 0b11 << 12;

    /// Broadcast Accept Mode
    pub const BAM: u32 = 1 << 15;

    // Buffer sizes
    pub const BSIZE_256: u32 = 0b11 << 16;
    pub const BSIZE_512: u32 = 0b10 << 16;
    pub const BSIZE_1024: u32 = 0b01 << 16;
    pub const BSIZE_2048: u32 = 0b00 << 16;
    pub const BSIZE_4096: u32 = (0b11 << 16) | (0b1 << 25);
    pub const BSIZE_8192: u32 = (0b10 << 16) | (0b1 << 25);
    pub const BSIZE_16384: u32 = (0b01 << 16) | (0b1 << 25);

    /// VLAN Filter Enable
    pub const VFE: u32 = 1 << 18;
    /// Canonical Form Indicator Enable
    pub const CFIEN: u32 = 1 << 19;
    /// Canonical Form Indicator
    pub const CFI: u32 = 1 << 20;
    /// Discard Pause Frames
    pub const DPF: u32 = 1 << 22;
    /// Pass MAC Control Frames
    pub const PMCF: u32 = 1 << 23;
    /// Strip Ethernet CRC
    pub const SECRC: u32 = 1 << 26;
}

/// TCTL (Transmit Control) register flags
pub struct TctlFlags;

#[allow(dead_code)]
impl TctlFlags {
    /// Transmit Enable
    pub const EN: u32 = 1 << 1;
    /// Pad Short Packets
    pub const PSP: u32 = 1 << 3;
    /// Collision Threshold shift
    pub const CT_SHIFT: u32 = 4;
    /// Collision Distance shift
    pub const COLD_SHIFT: u32 = 12;
    /// Software XOFF Transmission
    pub const SWXOFF: u32 = 1 << 22;
    /// Retransmit on Late Collision
    pub const RTLC: u32 = 1 << 24;
    /// Unordered Transmit
    pub const UNORTX: u32 = 1 << 25;

    // TX Descriptor Minimum Threshold
    pub const TXDMT_0: u32 = 0b00 << 26;
    pub const TXDMT_1: u32 = 0b01 << 26;
    pub const TXDMT_2: u32 = 0b10 << 26;
    pub const TXDMT_3: u32 = 0b11 << 26;

    /// Re-transmit on late collision (no threshold)
    pub const RR_NOTHRESH: u32 = 0b11 << 29;
}

/// CTRL (Device Control) register flags
pub struct CtrlFlags;

impl CtrlFlags {
    /// Device Reset
    pub const RST: u32 = 1 << 26;
}

/// Interrupt Cause flags
pub struct IntFlags;

#[allow(dead_code)]
impl IntFlags {
    /// Transmit Descriptor Written Back
    pub const TXDW: u32 = 1 << 0;
    /// Transmit Queue Empty
    pub const TXQE: u32 = 1 << 1;
    /// Link Status Change
    pub const LSC: u32 = 1 << 2;
    /// Receive Sequence Error
    pub const RXSEQ: u32 = 1 << 3;
    /// Receive Descriptor Minimum Threshold Reached
    pub const RXDMT0: u32 = 1 << 4;
    /// Receiver Overrun
    pub const RXO: u32 = 1 << 6;
    /// Receive Timer Interrupt
    pub const RXT0: u32 = 1 << 7;
}
