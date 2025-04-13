use super::{RawGenericAddress, SdtHeader};
use crate::{
    drivers::acpi::{AcpiRevision, sdt::Sdt as _},
    impl_sdt,
};

impl_sdt!(Fadt);

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
/// <https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#fixed-acpi-description-table-fadt>
struct MinimalFadt {
    header: SdtHeader,
    firmware_ctrl: u32,
    dsdt: u32,

    /// Used in ACPI 1.0 only
    int_model: u8,

    preferred_pm_profile: u8,
    sci_int: u16,
    smi_cmd: u32,
    acpi_enable: u8,
    acpi_disable: u8,
    s4bios_req: u8,
    pstate_cnt: u8,
    pm1a_evt_blk: u32,
    pm1b_evt_blk: u32,
    pm1a_cnt_blk: u32,
    pm1b_cnt_blk: u32,
    pm2_cnt_blk: u32,
    pm_tmr_blk: u32,
    gpe0_blk: u32,
    gpe1_blk: u32,
    pm1_evt_len: u8,
    pm1_cnt_len: u8,
    pm2_cnt_len: u8,
    pm_tmr_len: u8,
    gpe0_blk_len: u8,
    gpe1_blk_len: u8,
    gpe1_base: u8,
    cstate_ctrl: u8,
    lvl2_latency: u16,
    lvl3_latency: u16,
    flush_size: u16,
    flush_stride: u16,
    duty_offset: u8,
    duty_width: u8,
    day_alarm: u8,
    month_alarm: u8,
    century: u8,

    /// Reserved in ACPI 1.0; used since ACPI 2.0+
    iapc_boot_arch: u16,

    _reserved2: u8,
    flags: u32,

    reset_reg: RawGenericAddress,

    reset_value: u8,
    arm_boot_arch: u16,
    fadt_minor_version: u8,
}

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
/// <https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#fixed-acpi-description-table-fadt>
struct FullFadt {
    minimal: MinimalFadt,

    // Only available in ACPI 2.0+
    x_firmware_ctrl: u64,
    x_dsdt: u64,

    x_pm1a_evt_blk: RawGenericAddress,
    x_pm1b_evt_blk: RawGenericAddress,
    x_pm1a_cnt_blk: RawGenericAddress,
    x_pm1b_cnt_blk: RawGenericAddress,
    x_pm2_cnt_blk: RawGenericAddress,
    x_pm_tmr_blk: RawGenericAddress,
    x_gpe0_blk: RawGenericAddress,
    x_gpe1_blk: RawGenericAddress,
    sleep_control_reg: RawGenericAddress,
    sleep_status_reg: RawGenericAddress,
    hypervisor_vendor_id: u64,
}

impl Fadt {
    #[must_use]
    pub fn parse(&self) -> ParsedFadt {
        assert!(usize::try_from(self.length()).unwrap() >= size_of::<MinimalFadt>());
        let minimal_fadt_ptr = self.start_vaddr.as_ptr::<MinimalFadt>();
        let minimal_fadt = unsafe { minimal_fadt_ptr.read() };

        // Do NOT use if ACPI is in version 1.0
        let _full_fadt_ptr = self.start_vaddr.as_ptr::<FullFadt>();

        let acpi_rev = super::super::ACPI_REVISION.load();

        // assert_eq!(self.revision(), 1, "FADT revision must be 1");
        // TODO: Parse and validate minor version

        // TODO: Parse the FADT

        let ps2_keyboard = match acpi_rev {
            // ACPI 1.0: PS/2 keyboard is always supported.
            AcpiRevision::V1 => true,
            // ACPI 2.0: Support is reported in `iapc_boot_arch`
            AcpiRevision::V2 => minimal_fadt.iapc_boot_arch & (1 << 1) != 0,
        };

        ParsedFadt { ps2_keyboard }
    }
}

pub struct ParsedFadt {
    ps2_keyboard: bool,
}

impl ParsedFadt {
    #[must_use]
    #[inline]
    pub const fn ps2_keyboard(&self) -> bool {
        self.ps2_keyboard
    }
}
