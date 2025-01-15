// FIXME: Support for multiple HPET blocks?

use x86_64::structures::paging::{PageTableFlags, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

use crate::boot::acpi::sdt::hpet_table::ParsedHpetTable;
use crate::mem::page_alloc::pmap::{self, PhysicalMapping};
use hyperdrive::locks::mcs::MUMcsLock;

static HPET: MUMcsLock<Hpet> = MUMcsLock::uninit();

#[derive(Debug)]
/// High Precision Event Timer (HPET) configuration.
pub struct Hpet {
    /// General Capabilities register.
    general_capabilities: GeneralCapabilities,
    /// General Configuration register.
    general_configuration: GeneralConfiguration,
    /// General Interrupt Status register.
    general_interrupt_status: GeneralInterruptStatus,
    /// Main Counter Value register.
    main_counter_value: MainCounterValue,
    /// Timers Configuration and Capabilities registers.
    timers_config_cap: [Option<TimerConfigCap>; 32],
    /// Timers Comparator Value registers.
    timers_comp_value: [Option<TimerCompValue>; 32],
}

impl Hpet {
    #[must_use]
    #[inline]
    pub const fn general_capabilities(&self) -> &GeneralCapabilities {
        &self.general_capabilities
    }

    #[must_use]
    #[inline]
    pub const fn general_configuration(&self) -> &GeneralConfiguration {
        &self.general_configuration
    }

    #[must_use]
    #[inline]
    pub const fn general_interrupt_status(&self) -> &GeneralInterruptStatus {
        &self.general_interrupt_status
    }

    #[must_use]
    #[inline]
    pub const fn main_counter_value(&self) -> &MainCounterValue {
        &self.main_counter_value
    }
}

macro_rules! read_write_reg {
    ($name:ident { $($field_name:ident : $field_type:ty),* $(,)? }) => {
        #[derive(Debug)]
        /// HPET Read/Write register
        pub struct $name {
            vaddr: VirtAddr,
            _physical_mapping: PhysicalMapping,
            $(
                $field_name: $field_type,
            )*
        }

        impl $name {
            /// Loads the register from the physical address.
            ///
            /// Does NOT validate the content of the register.
            fn new(paddr: PhysAddr, $($field_name: $field_type),*) -> Self {
                let flags = pmap::FLAGS_MMIO;

                let physical_mapping = PhysicalMapping::new(paddr, core::mem::size_of::<u64>(), flags);
                let vaddr = physical_mapping.translate(paddr).unwrap();
                Self {
                    vaddr,
                    _physical_mapping: physical_mapping,
                    $(
                        $field_name,
                    )*
                }
            }

            /// Get register value
            const fn read(&self) -> u64 {
                unsafe { self.vaddr.as_ptr::<u64>().read() }
            }

            /// Use only to write to the register
            const fn as_mut(&mut self) -> &mut u64 {
                unsafe { &mut *self.vaddr.as_mut_ptr::<u64>() }
            }
        }
    };
}

// As it is read-only, its value won't change.
// So we can just copy instead of handling physical mappings.
#[derive(Debug, Clone, Copy)]
/// HPET Read-only register
pub struct GeneralCapabilities(u64);

impl GeneralCapabilities {
    #[must_use]
    pub fn new(paddr: PhysAddr) -> Self {
        let flags = PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE;

        let physical_mapping =
            PhysicalMapping::<Size4KiB>::new(paddr, core::mem::size_of::<u64>(), flags);
        let vaddr = physical_mapping.translate(paddr).unwrap();
        Self(unsafe { vaddr.as_ptr::<u64>().read() })
    }

    #[must_use]
    #[inline]
    /// The period of the HPET in femtoseconds.
    pub fn period(self) -> u32 {
        let period = self.0 >> 32;
        assert!(period <= 0x05F5_E100);
        assert_ne!(period, 0);
        u32::try_from(period).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn num_timers(self) -> u8 {
        u8::try_from((self.0 >> 8) & 0b1_1111).unwrap()
    }

    #[must_use]
    #[inline]
    pub const fn count_size_capable(self) -> bool {
        (self.0 >> 13) & 1 == 1
    }

    fn validate(self, hpet_info: &ParsedHpetTable) {
        assert_eq!(
            hpet_info.comparator_count(),
            self.num_timers(),
            "HPET comparator count mismatch"
        );
        assert_eq!(
            hpet_info.count_size_capable(),
            self.count_size_capable(),
            "HPET count size capability mismatch"
        );
        assert!(self.period() <= 100_000_000, "HPET period too large");
    }
}

read_write_reg!(GeneralConfiguration {});

impl GeneralConfiguration {
    #[must_use]
    #[inline]
    pub const fn get_enable_cnf(&self) -> bool {
        self.read() & 1 != 0
    }

    #[inline]
    pub const fn set_enable_cnf(&mut self, enable: bool) {
        let ptr = self.as_mut();
        if enable {
            *ptr |= 1;
        } else {
            *ptr &= !1;
        }
    }

    #[must_use]
    #[inline]
    pub const fn legacy_replacement(&self) -> bool {
        (self.read() >> 1) & 1 == 1
    }

    #[inline]
    pub const fn set_legacy_replacement(&mut self, enable: bool) {
        let ptr = self.as_mut();
        if enable {
            *ptr |= 1 << 1;
        } else {
            *ptr &= !(1 << 1);
        }
    }

    fn validate(&self) {
        assert!(!self.get_enable_cnf(), "HPET enabled");
    }
}

read_write_reg!(GeneralInterruptStatus { nb_timers: u8 });

impl GeneralInterruptStatus {
    #[must_use]
    #[inline]
    /// Reads the interrupt status of the timer.
    ///
    /// ## Safety
    ///
    /// The caller ensure the timer is valid.
    pub const fn get_tn_int_status(&self, timer: u8) -> bool {
        assert!(timer < 32 && timer < self.nb_timers);
        self.read() & (1 << timer) != 0
    }

    #[inline]
    /// Clears the interrupt status of the timer.
    ///
    /// ## Safety
    ///
    /// The caller ensure the timer is valid.
    pub const unsafe fn clear_tn_int_status(&mut self, timer: u8) {
        assert!(timer < 32 && timer < self.nb_timers);
        let ptr = self.as_mut();
        *ptr |= 1 << timer;
    }

    #[inline]
    pub fn validate(&self) {
        assert_eq!(self.read(), 0, "HPET interrupt status not null");
    }
}

read_write_reg!(MainCounterValue { count_cap: bool });

impl MainCounterValue {
    #[must_use]
    #[inline]
    pub const fn get_value(&self) -> u64 {
        // FIXME: Handle 32-bit counter ? Does it exist on x86_64 ?
        assert!(self.count_cap, "HPET count size not capable");
        self.read()
    }

    fn validate(&self) {
        assert!(self.get_value() == 0, "HPET main counter non null");
    }
}

read_write_reg!(TimerConfigCap { timer: u8 });

impl TimerConfigCap {
    #[must_use]
    #[inline]
    /// ## WARNING
    ///
    /// According the `OSDev` wiki, this field can be little bit misleading :
    ///
    /// "Keep in mind that allowed interrupt routing may be insane. Namely,
    /// you probably want to use some of ISA interrupts - or, at very least,
    /// be able to use them at one point unambiguously.
    /// Last time I checked `VirtualBox` allowed mappings for HPET,
    /// it allowed every timer to be routed to any of 32 I/O APIC inputs present on the system.
    /// Knowing how buggy hardware can be,
    /// I wouldn't be too surprised if there exists a PC with HPET claiming that input #31 is allowed,
    /// when there are only 24 I/O APIC inputs. Be aware of this when choosing interrupt routing for timers."
    pub const fn int_route_cap(&self, irq: u8) -> bool {
        assert!(irq < 32);
        (self.read() >> 32) & (1 << irq) == 1
    }

    #[must_use]
    #[inline]
    pub const fn fsb_int_map_cap(&self) -> bool {
        (self.read() >> 15) & 1 == 1
    }

    #[must_use]
    #[inline]
    pub const fn get_fsb_int_map(&self) -> bool {
        (self.read() >> 14) & 1 == 1
    }

    #[inline]
    pub fn set_fsb_int_map(&mut self, value: bool) {
        if value {
            assert!(
                self.fsb_int_map_cap(),
                "HPET timer {} FSB interrupt mapping not capable",
                self.timer
            );
            *self.as_mut() |= 1 << 14;
        } else {
            *self.as_mut() &= !(1 << 14);
        }
    }

    #[must_use]
    #[inline]
    pub fn get_interrupt_rout(&self) -> u8 {
        u8::try_from((self.read() >> 9) & 0b1_1111).unwrap()
    }

    #[inline]
    pub fn set_interrupt_rout(&mut self, value: u8) {
        assert!(
            value < 32,
            "HPET timer {} FSB interrupt enable out of range",
            self.timer
        );
        let ptr = self.as_mut();
        *ptr &= !(0b1_1111 << 9); // Clear the field
        *ptr |= u64::from(value) << 9; // Set the new value
    }

    #[must_use]
    #[inline]
    pub const fn get_mode_32_bits(&self) -> bool {
        (self.read() >> 8) & 1 == 1
    }

    // TODO: 6 Tn_VAL_SET_CNF (used only in periodic mode)

    #[must_use]
    #[inline]
    pub const fn size_cap(&self) -> bool {
        (self.read() >> 5) & 1 == 1
    }

    #[must_use]
    #[inline]
    pub const fn periodic_cap(&self) -> bool {
        (self.read() >> 4) & 1 == 1
    }

    #[must_use]
    #[inline]
    pub const fn get_periodic_mode(&self) -> bool {
        (self.read() >> 3) & 1 == 1
    }

    #[inline]
    pub fn set_periodic_mode(&mut self, value: bool) {
        if value {
            assert!(
                self.periodic_cap(),
                "HPET timer {} not periodic capable",
                self.timer
            );
            *self.as_mut() |= 1 << 3;
        } else {
            *self.as_mut() &= !(1 << 3);
        }
    }

    #[must_use]
    #[inline]
    /// Is triggering interrupts enabled for this timer ?
    pub const fn get_interrupts_trig(&self) -> bool {
        (self.read() >> 2) & 1 == 1
    }

    #[inline]
    /// Enable or disable triggering interrupts for this timer.
    ///
    /// Even if this bit is disabled, the timer will still set the corresponding bit
    /// in the General Interrupt Status register.
    pub fn set_interrupts_trig(&mut self, value: bool) {
        let ptr = self.as_mut();
        if value {
            *ptr |= 1 << 2;
        } else {
            *ptr &= !(1 << 2);
        }
    }

    #[must_use]
    #[inline]
    pub const fn get_int_type(&self) -> InterruptTriggerType {
        if (self.read() >> 1) & 1 == 1 {
            InterruptTriggerType::Level
        } else {
            InterruptTriggerType::Edge
        }
    }

    #[inline]
    pub fn set_int_type(&mut self, int_type: &InterruptTriggerType) {
        let ptr = self.as_mut();
        match int_type {
            InterruptTriggerType::Edge => *ptr &= !(1 << 1),
            InterruptTriggerType::Level => *ptr |= 1 << 1,
        }
    }

    fn validate(&self) {
        assert!(self.timer < 32, "HPET timer out of range");
        assert!(self.size_cap(), "HPET timer {} is 32-bit", self.timer);
        assert!(
            !self.get_mode_32_bits(),
            "HPET timer {} forced in 32-bit mode",
            self.timer
        );
    }
}

pub enum InterruptTriggerType {
    Edge,
    Level,
}

read_write_reg!(TimerCompValue { count_cap: bool });

impl TimerCompValue {
    #[must_use]
    pub const fn get_value(&self) -> u64 {
        // FIXME: Handle 32-bit counter ? Does it exist on x86_64 ?
        assert!(self.count_cap, "HPET count size not capable");
        self.read()
    }

    pub fn set_value(&mut self, value: u64) {
        assert!(self.count_cap, "HPET count size not capable");
        *self.as_mut() = value;
    }

    #[allow(clippy::unused_self)]
    const fn validate(&self) {}
}

pub fn init(hpet_info: &ParsedHpetTable) {
    assert_eq!(
        hpet_info.base_address().address_space(),
        crate::boot::acpi::sdt::AdressSpace::SystemMemory
    );

    // TODO: Only one mapping for the whole HPET block
    // see section 2.3.1 of the spec

    let general_capabilities =
        GeneralCapabilities::new(PhysAddr::new(hpet_info.general_capabilities().address()));
    general_capabilities.validate(hpet_info);
    crate::debug!("HPET period: {} ps", general_capabilities.period() / 1_000);
    if !hpet_info.count_size_capable() {
        crate::warn!("HPET count size not capable");
    }

    let mut general_configuration =
        GeneralConfiguration::new(PhysAddr::new(hpet_info.general_configuration().address()));
    general_configuration.validate();

    let general_interrupt_status = GeneralInterruptStatus::new(
        PhysAddr::new(hpet_info.general_interrupt_status().address()),
        general_capabilities.num_timers(),
    );
    general_interrupt_status.validate();

    let main_counter_value = MainCounterValue::new(
        PhysAddr::new(hpet_info.main_counter_value().address()),
        hpet_info.count_size_capable(),
    );
    main_counter_value.validate();

    let mut timers_config_cap = [const { None }; 32];
    let mut timers_comp_value = [const { None }; 32];
    for i in 0..hpet_info.comparator_count() {
        let timer_config_cap = TimerConfigCap::new(
            PhysAddr::new(hpet_info.timer_n_configuration(i).address()),
            i,
        );
        timer_config_cap.validate();
        // TODO: Add a "periodic capable" field to avoid re-reading it every time
        timers_config_cap[usize::from(i)] = Some(timer_config_cap);

        let timer_comp_value = TimerCompValue::new(
            PhysAddr::new(hpet_info.timer_n_comparator_value(i).address()),
            hpet_info.count_size_capable(),
        );
        timer_comp_value.validate();
        timers_comp_value[usize::from(i)] = Some(timer_comp_value);
    }

    // TODO: Initialize timers
    // That is, allocating IRQs for allowed interrupt routing.

    // Enable HPET
    general_configuration.set_enable_cnf(true);
    crate::debug!("HPET enabled");

    let hpet = Hpet {
        general_capabilities,
        general_configuration,
        general_interrupt_status,
        main_counter_value,
        timers_config_cap,
        timers_comp_value,
    };

    HPET.init(hpet);
}

impl Hpet {
    // TODO: Implement methods!
}

pub fn with_hpet<R>(f: impl FnOnce(&mut Hpet) -> R) -> R {
    HPET.with_locked(f)
}

pub fn main_counter_value() -> u64 {
    let hpet = unsafe { HPET.force_lock() };
    hpet.main_counter_value().get_value()
}
