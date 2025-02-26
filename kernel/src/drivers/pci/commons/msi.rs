//! Mesage Signaled Interrupts (MSI) support.

use crate::arch::interrupts::Irq;
use crate::locals;

use super::super::{PciHandler, commons::CapabilityHeader, iter_capabilities};

use super::PciAddress;

pub struct Msi {
    capability: MsiCapability,
}

impl Msi {
    #[must_use]
    pub fn new(handler: &mut dyn PciHandler, device: &super::Device) -> Option<Self> {
        let capability = MsiCapability::find(handler, device)?;

        Some(Self { capability })
    }

    // TODO: Use Multiple Message
    pub fn setup_int(&self, vector: Irq, handler: &mut dyn PciHandler) {
        let lapic_paddr = unsafe { locals!().lapic().force_lock().paddr() };
        let lapic_id = locals!().apic_id(); // TODO: Load balance between APs?

        let msg_addr = lapic_paddr.as_u64() | (u64::from(lapic_id) << 12);
        let low_dword = u32::try_from(msg_addr & 0xFFFF_FFFC).unwrap();
        let high_dword = u32::try_from(msg_addr >> 32).unwrap();
        assert!(high_dword == 0 || self.capability.qword_addressing);

        let message_addr_base = PciAddress::new(
            self.capability.base.sbdf.segment(),
            self.capability.base.sbdf.bus(),
            self.capability.base.sbdf.device(),
            self.capability.base.sbdf.function(),
            0x4,
        );
        handler.write_raw(message_addr_base, low_dword);

        if self.capability.qword_addressing {
            let upper_message_addr_base = PciAddress::new(
                self.capability.base.sbdf.segment(),
                self.capability.base.sbdf.bus(),
                self.capability.base.sbdf.device(),
                self.capability.base.sbdf.function(),
                0x8,
            );
            handler.write_raw(upper_message_addr_base, high_dword);
        }

        let message_data_base = PciAddress::new(
            self.capability.base.sbdf.segment(),
            self.capability.base.sbdf.bus(),
            self.capability.base.sbdf.device(),
            self.capability.base.sbdf.function(),
            if self.capability.qword_addressing {
                0xC
            } else {
                0x8
            },
        );

        let message_data = vector as u16;
        // FIXME: If not Extended Message Capable, writing to the upper DWORD is not allowed
        handler.write_raw(message_data_base, u32::from(message_data));
    }

    pub fn enable(&self, handler: &mut dyn PciHandler) {
        let mut first_dword = handler.read_raw(self.capability.base);

        // Enable MSI
        first_dword |= 1 << 16;

        handler.write_raw(self.capability.base, first_dword);
    }
}

pub struct MsiCapability {
    base: PciAddress,
    /// Number of messages that the device is capable of generating
    multiple_message_capable: u8,
    qword_addressing: bool,
    pvm_capable: bool,
    extended_message_capable: bool,
}

impl MsiCapability {
    #[must_use]
    pub fn find(handler: &mut dyn PciHandler, device: &super::Device) -> Option<Self> {
        let c = iter_capabilities(handler, device).find(|c| c.id() == CapabilityHeader::ID_MSI)?;

        let first_dword = handler.read_raw(c.pci_addr());
        let msg_control = MessageControlValue {
            raw: u16::try_from(first_dword >> 16).unwrap(),
        };

        Some(Self {
            base: c.pci_addr(),
            multiple_message_capable: msg_control.multiple_message_capable(),
            qword_addressing: msg_control.qword_addressing(),
            pvm_capable: msg_control.pvm_capable(),
            extended_message_capable: msg_control.extended_message_capable(),
        })
    }
}

struct MessageControlValue {
    raw: u16,
}

impl MessageControlValue {
    #[must_use]
    #[inline]
    /// Returns the number of messages that the device is capable of generating.
    pub const fn multiple_message_capable(&self) -> u8 {
        match self.raw & 0b1110 {
            0b0000 => 1,
            0b0010 => 2,
            0b0100 => 4,
            0b0110 => 8,
            0b1000 => 16,
            0b1010 => 32,
            0b1100 | 0b1110 => panic!("Invalid multiple message capable value"),
            _ => unreachable!(),
        }
    }

    #[must_use]
    #[inline]
    pub const fn qword_addressing(&self) -> bool {
        self.raw & (1 << 7) != 0
    }

    #[must_use]
    #[inline]
    pub const fn pvm_capable(&self) -> bool {
        self.raw & (1 << 8) != 0
    }

    #[must_use]
    #[inline]
    pub const fn extended_message_capable(&self) -> bool {
        self.raw & (1 << 9) != 0
    }
}
