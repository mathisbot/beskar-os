use super::{GenericAddress, RawGenericAddress, SdtHeader};
use crate::drivers::acpi::{AcpiRevision, sdt::Sdt as _};
use beskar_core::arch::PhysAddr;

pub mod reg;

super::impl_sdt!(Fadt);

/// Select a preferred field and dynamically fallback to a minimal field if the preferred field is not available.
/// This is used to parse the FADT structure, which has a minimal version and an extended version.
/// The minimal version is always present, but the extended version is only present in ACPI 2.0+.
///
/// The macro takes the following arguments:
/// - `full`: The full FADT structure, which may be `None` if the extended version is not present.
/// - `minimal`: The minimal FADT structure, which is always present.
/// - `field`: The field in the minimal FADT structure.
/// - `x_field`: The field in the full FADT structure.
/// - `convert_full`: A fonction (or closure) to convert the full field to the desired type.
/// - `convert_minimal`: A function (or closure) to convert the minimal field to the desired type.
/// - `null_cond`: A function (or closure) to check if the full field is null.
macro_rules! fallback_field {
    (
        minimal = $minimal:expr,
        field = $field:ident,
        convert_minimal = $convert_minimal:expr,
        full = $full:expr,
        x_field = $x_field:ident,
        convert_full = $convert_full:expr,
        null_cond = $null_cond:expr,
    ) => {{
        $full
            .as_ref()
            .and_then(|f| (!($null_cond(f.$x_field))).then(|| ($convert_full(f.$x_field))))
            .unwrap_or_else(|| ($convert_minimal($minimal.$field)))
    }};
}

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
/// <https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#fixed-acpi-description-table-fadt>
struct MinimalFadt {
    header: SdtHeader,
    firmware_ctrl: u32,
    /// DSDT Physical Address
    ///
    /// Invalid if `FullFadt` is available and field `x_dsdt` is non-zero.
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
        let acpi_rev = super::super::ACPI_REVISION.load();

        // This minimal FADT structure is always present, even in ACPI 1.0.
        // However, some fields are invalidated if full FADT is available.
        let minimal_fadt = {
            let minimal_fadt_ptr = self.start_vaddr.as_ptr::<MinimalFadt>();
            unsafe { minimal_fadt_ptr.read() }
        };

        // If ACPI 2.0+ is available, the FADT is extended with a full FADT structure.
        let full_fadt = {
            let full_fadt_valid = self.length() >= size_of::<FullFadt>().try_into().unwrap();
            if full_fadt_valid {
                let full_fadt_ptr = self.start_vaddr.as_ptr::<FullFadt>();
                Some(unsafe { full_fadt_ptr.read() })
            } else {
                None
            }
        };

        // assert_eq!(self.revision(), 1, "FADT revision must be 1");
        // TODO: Parse and validate minor version

        // TODO: Parse the FADT

        let ps2_keyboard = match acpi_rev {
            // ACPI 1.0: PS/2 keyboard is always supported.
            AcpiRevision::V1 => true,
            // ACPI 2.0: Support is reported in `iapc_boot_arch`
            AcpiRevision::V2 => minimal_fadt.iapc_boot_arch & (1 << 1) != 0,
        };

        let dsdt = fallback_field!(
            minimal = minimal_fadt,
            field = dsdt,
            convert_minimal = |x| PhysAddr::new(u64::from(x)),
            full = full_fadt,
            x_field = x_dsdt,
            convert_full = PhysAddr::new,
            null_cond = |x| x == 0,
        );

        let pm1_cnt = {
            let pm1a_cnt_blk = fallback_field!(
                minimal = minimal_fadt,
                field = pm1a_cnt_blk,
                convert_minimal = |x| GenericAddress {
                    address_space: super::AddressSpace::SystemIO,
                    bit_width: 16,
                    bit_offset: 0,
                    access_size: super::AccessSize::Byte,
                    address: u64::from(x),
                },
                full = full_fadt,
                x_field = x_pm1a_cnt_blk,
                convert_full = GenericAddress::from,
                null_cond = |x: RawGenericAddress| x.access_size() == 0
                    && x.address() == 0
                    && x.bit_width() == 0
                    && x.bit_offset() == 0
                    && x.address_space() == 0,
            );

            let pm1b_cnt_blk = fallback_field!(
                minimal = minimal_fadt,
                field = pm1b_cnt_blk,
                convert_minimal = |x| (x != 0).then(|| GenericAddress {
                    address_space: super::AddressSpace::SystemIO,
                    bit_width: 16,
                    bit_offset: 0,
                    access_size: super::AccessSize::Byte,
                    address: u64::from(x),
                }),
                full = full_fadt,
                x_field = x_pm1b_cnt_blk,
                convert_full = |x: RawGenericAddress| Some(GenericAddress::from(x)),
                null_cond = |x: RawGenericAddress| x.access_size() == 0
                    && x.address() == 0
                    && x.bit_width() == 0
                    && x.bit_offset() == 0
                    && x.address_space() == 0,
            );

            reg::Pm1ControlRegister::new(pm1a_cnt_blk, pm1b_cnt_blk)
        };

        ParsedFadt {
            ps2_keyboard,
            dsdt,
            pm1_cnt,
        }
    }
}

pub struct ParsedFadt {
    ps2_keyboard: bool,
    dsdt: PhysAddr,
    pm1_cnt: reg::Pm1ControlRegister,
}

impl ParsedFadt {
    #[must_use]
    #[inline]
    pub const fn ps2_keyboard(&self) -> bool {
        self.ps2_keyboard
    }

    #[must_use]
    #[inline]
    pub const fn dsdt(&self) -> PhysAddr {
        self.dsdt
    }

    #[must_use]
    #[inline]
    pub const fn pm1_cnt(&self) -> &reg::Pm1ControlRegister {
        &self.pm1_cnt
    }
}
