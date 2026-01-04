//! ELF binary loader module
//!
//! Provides functionality to load ELF binaries into memory.
//!
//! # Usage
//!
//! Use the `load` function to load an ELF binary from a byte slice.
//!
//! ```rust
//! # use elf::{ElfLoader, MemoryMapper, mapper::{MappedRegion, VirtAddr}, PageFlags};
//! #
//! # #[derive(Debug, Default)]
//! # /// Mock memory mapper for testing
//! # struct MockMapper {
//! #     regions: Vec<(VirtAddr, u64)>,
//! # }
//! #
//! # impl MemoryMapper for MockMapper {
//! #     fn map_region(
//! #         &mut self,
//! #         size: u64,
//! #         _flags: PageFlags,
//! #     ) -> core::result::Result<MappedRegion, ()> {
//! #         let virt_addr = if self.regions.is_empty() {
//! #             VirtAddr::new_extend(0x1000)
//! #         } else {
//! #             let (last_start, last_size) = self.regions.last().unwrap();
//! #             *last_start + *last_size
//! #         };
//! #
//! #         self.regions.push((virt_addr, size));
//! #
//! #         Ok(MappedRegion { virt_addr, size })
//! #     }
//! #
//! #     fn copy_data(&mut self, _dest: VirtAddr, _src: &[u8]) -> core::result::Result<(), ()> {
//! #         Ok(())
//! #     }
//! #
//! #     fn zero_region(&mut self, _dest: VirtAddr, _size: u64) -> core::result::Result<(), ()> {
//! #         Ok(())
//! #     }
//! #
//! #     fn update_flags(
//! #         &mut self,
//! #         _region: MappedRegion,
//! #         _flags: PageFlags,
//! #     ) -> core::result::Result<(), ()> {
//! #         Ok(())
//! #     }
//! #
//! #     fn unmap_region(&mut self, _region: MappedRegion) -> core::result::Result<(), ()> {
//! #         Ok(())
//! #     }
//! #
//! #     fn rollback(&mut self) {}
//! # }
//! #
//! let binary_data: &[u8] = &[/* Binary data */];
//! let mut mapper = MockMapper::default(); // Initialize your memory mapper
//! let res = ElfLoader::load(binary_data, &mut mapper);
//!
//! match res {
//!     Ok(bin) => {
//!         // Use the loaded binary
//!     },
//!     Err(e) => {
//!         // Handle loading error
//!     },
//! }
//! ```

#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![no_std]

extern crate alloc;

mod error;
mod loader;
pub mod mapper;
pub mod segments;

pub use error::ElfLoadError;
pub use loader::ElfLoader;
pub use mapper::{MemoryMapper, PageFlags};
pub use segments::TlsTemplate;

/// Result type for ELF loading operations
pub type Result<T> = core::result::Result<T, ElfLoadError>;
