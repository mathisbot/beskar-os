use beskar_core::mem::ranges::MemoryRange;

mod address_space;
mod frame_alloc;
mod heap;
mod page_alloc;
mod phys_map;
pub mod vmm;

pub use address_space::AddressSpace;

pub fn init(recursive_index: u16, regions: &[MemoryRange]) {
    frame_alloc::init(regions);
    vmm::init(recursive_index);
    heap::init();
}
