//! File Allocation Table (FAT32) file system implementation.
mod bpb;

const BUFFER_SIZE: usize = 1024;

const DIR_ENTRY_SIZE: usize = 32;
