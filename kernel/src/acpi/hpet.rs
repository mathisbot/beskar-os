use super::sdt::hpet_table::ParsedHpetTable;

pub struct Hpet {}

pub fn init(hpet_info: ParsedHpetTable) {
    assert_eq!(
        hpet_info.base_address().address_space(),
        super::sdt::AdressSpace::SystemMemory
    );
    // TODO: Initialize HPET
}
