# ELF Loader

A generic, architecture-independent ELF binary loader with pluggable memory mapping abstractions.

## Features

- **Generic Memory Mapping**: Implements custom memory mapping via trait-based abstractions
- **No Standard Library**: `no_std` compatible
- **Error Handling**: Returns errors instead of panicking on invalid inputs

## Usage

```rust
use elf::{ElfLoader, MemoryMapper, PageFlags};

// Implement MemoryMapper for your OS/architecture
struct MyMapper {
    // ... mapper state
}

impl MemoryMapper for MyMapper {
    fn map_region(&mut self, size: u64, flags: PageFlags) -> Result<MappedRegion, ()> {
        // Allocate and map memory
    }
    
    fn copy_data(&mut self, dest: u64, src: &[u8]) -> Result<(), ()> {
        // Copy data to mapped region
    }
    
    // ... implement other methods
}

// Load an ELF binary
let mut mapper = MyMapper::new();
let binary = ElfLoader::load(elf_data, &mut mapper)?;
```
