//! File Allocation Table (FAT) file system implementation.
mod bs;
pub mod date;
mod dirent;

/// Fat types
pub type FatType = FatUnion<(), (), ()>;

pub enum FatUnion<T12, T16, T32> {
    Fat12(T12),
    Fat16(T16),
    Fat32(T32),
}
