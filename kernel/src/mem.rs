use bootloader::info::MemoryRegion;
use x86_64::VirtAddr;

pub mod address_space;
pub mod frame_alloc;
pub mod heap;
pub mod page_alloc;
pub mod page_table;
pub mod ranges;

pub fn init(recursive_index: u16, regions: &[MemoryRegion], kernel_vaddr: VirtAddr) {
    page_table::init(recursive_index);

    frame_alloc::init(regions);

    page_alloc::init(recursive_index);

    address_space::init(recursive_index, kernel_vaddr);

    heap::init();
}
