use bootloader::info::MemoryRegion;

pub mod address_space;
pub mod frame_alloc;
pub mod heap;
pub mod page_alloc;
pub mod page_table;
pub mod ranges;

pub fn init(recursive_index: u16, regions: &[MemoryRegion]) {
    page_table::init(recursive_index);

    frame_alloc::init(regions);

    page_alloc::init(recursive_index);

    address_space::init(recursive_index);

    heap::init();
}
