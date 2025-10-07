//! File Allocation Table (FAT) file system implementation.
use super::FileSystem;
use beskar_core::storage::BlockDevice;
use thiserror::Error;

pub mod bs;
pub mod date;
pub mod dir;
pub mod dirent;
#[expect(clippy::module_inception, reason = "FS is named after this table")]
pub mod fat;
pub mod file;

/// Fat types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatType {
    Fat12,
    Fat16,
    Fat32,
}

impl FatType {
    #[must_use]
    #[inline]
    pub const fn cluster_size_bits(self) -> u32 {
        Cluster::size_bits(self)
    }
}

#[derive(Debug, Clone)]
pub enum FatUnion<T12, T16, T32> {
    Fat12(T12),
    Fat16(T16),
    Fat32(T32),
}

impl<T12, T16, T32> FatUnion<T12, T16, T32> {
    #[must_use]
    #[inline]
    pub const fn fat_type(&self) -> FatType {
        match self {
            Self::Fat12(_) => FatType::Fat12,
            Self::Fat16(_) => FatType::Fat16,
            Self::Fat32(_) => FatType::Fat32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cluster(u32);

impl Cluster {
    const SIZE_FAT12: u32 = 12;
    const SIZE_FAT16: u32 = 16;
    const SIZE_FAT32: u32 = 32;

    #[must_use]
    #[inline]
    pub const fn new(cluster: u32) -> Self {
        Self(cluster)
    }

    #[must_use]
    #[inline]
    pub const fn value(&self) -> u32 {
        self.0
    }

    #[must_use]
    #[inline]
    pub const fn size_bits(fat_type: FatType) -> u32 {
        match fat_type {
            FatType::Fat12 => Self::SIZE_FAT12,
            FatType::Fat16 => Self::SIZE_FAT16,
            FatType::Fat32 => Self::SIZE_FAT32,
        }
    }

    #[must_use]
    #[inline]
    pub const fn is_valid(&self, fat_type: FatType) -> bool {
        match fat_type {
            FatType::Fat12 => self.0 >= 2 && self.0 <= 0xFF6,
            FatType::Fat16 => self.0 >= 2 && self.0 <= 0xFFF6,
            FatType::Fat32 => self.0 >= 2 && self.0 <= 0x0FFF_FFF6,
        }
    }

    #[must_use]
    #[inline]
    pub const fn is_end_of_chain(&self, fat_type: FatType) -> bool {
        match fat_type {
            FatType::Fat12 => self.0 >= 0xFF8 && self.0 <= 0xFFF,
            FatType::Fat16 => self.0 >= 0xFFF8 && self.0 <= 0xFFFF,
            FatType::Fat32 => self.0 >= 0x0FF_FFFF8 && self.0 <= 0x0FFF_FFFF,
        }
    }

    #[must_use]
    #[inline]
    pub const fn is_bad(&self, fat_type: FatType) -> bool {
        match fat_type {
            FatType::Fat12 => self.0 == 0xFF7,
            FatType::Fat16 => self.0 == 0xFFF7,
            FatType::Fat32 => self.0 == 0x0FFF_FFF7,
        }
    }

    #[must_use]
    #[inline]
    pub const fn is_free(&self) -> bool {
        self.0 == 0
    }

    #[must_use]
    #[inline]
    pub const fn is_reserved(&self, fat_type: FatType) -> bool {
        match fat_type {
            FatType::Fat12 => self.0 == 0x0FF0 || self.0 == 0x0FF6,
            FatType::Fat16 => self.0 == 0xFFF0 || self.0 == 0xFFF6,
            FatType::Fat32 => self.0 == 0x0FFF_FFF0 || self.0 == 0x0FFF_FFF6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
/// Error type for FAT filesystem operations
pub enum FatError {
    #[error("Invalid parameter")]
    InvalidParameter,
    #[error("I/O error")]
    Io,
    #[error("Not found")]
    NotFound,
    #[error("Invalid filesystem")]
    InvalidFilesystem,
    #[error("Invalid boot sector")]
    InvalidBootSector,
    #[error("Invalid FAT entry")]
    InvalidFat,
    #[error("Invalid cluster")]
    InvalidCluster,
    #[error("Invalid directory entry")]
    InvalidDirEntry,
    #[error("Out of bounds")]
    OutOfBounds,
    #[error("Invalid file name")]
    NotSupported,
    #[error("Unexpected end of file")]
    UnexpectedEOF,
}

pub type FatResult<T> = Result<T, FatError>;

type BoxedDataReader<'a> =
    alloc::boxed::Box<dyn FnMut(Cluster, u32, &mut [u8]) -> FatResult<()> + 'a>;
type RefDataReader<'a> = &'a mut dyn FnMut(Cluster, u32, &mut [u8]) -> FatResult<()>;
type RefDataWriter<'a> = &'a mut dyn FnMut(Cluster, u32, &[u8]) -> FatResult<()>;

pub struct FatFs<D: BlockDevice> {
    device: D,
    fat_type: FatType,
    fat_size: u32,
    data_size: u32,
    data_start: u32,
    data_end: u32,
}

impl<D: BlockDevice> FileSystem for FatFs<D> {
    fn close(&mut self, path: super::Path) -> super::FileResult<()> {
        // No-op for FAT
        Ok(())
    }

    fn open(&mut self, path: super::Path) -> super::FileResult<()> {
        // No-op for FAT
        Ok(())
    }

    fn create(&mut self, path: super::Path) -> super::FileResult<()> {
        todo!("Create file in FAT filesystem");
    }

    fn delete(&mut self, path: super::Path) -> super::FileResult<()> {
        todo!("Delete file in FAT filesystem");
    }

    fn exists(&mut self, path: super::Path) -> super::FileResult<bool> {
        todo!("Check if file exists in FAT filesystem");
    }

    fn read(
        &mut self,
        path: super::Path,
        buffer: &mut [u8],
        offset: usize,
    ) -> super::FileResult<usize> {
        todo!("Read file from FAT filesystem");
    }

    fn write(
        &mut self,
        path: super::Path,
        buffer: &[u8],
        offset: usize,
    ) -> super::FileResult<usize> {
        todo!("Write file to FAT filesystem");
    }

    fn metadata(&mut self, path: super::Path) -> super::FileResult<super::FileMetadata> {
        todo!("Get file metadata from FAT filesystem");
    }

    fn read_dir(
        &mut self,
        path: super::Path,
    ) -> super::FileResult<alloc::vec::Vec<super::PathBuf>> {
        todo!("Read directory from FAT filesystem");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fat_union() {
        type DummyFatUnit = FatUnion<u32, u32, u32>;

        // Test FatUnion with simple types
        let fat12 = DummyFatUnit::Fat12(12u32);
        let fat16 = DummyFatUnit::Fat16(16u32);
        let fat32 = DummyFatUnit::Fat32(32u32);

        assert_eq!(fat12.fat_type(), FatType::Fat12);
        assert_eq!(fat16.fat_type(), FatType::Fat16);
        assert_eq!(fat32.fat_type(), FatType::Fat32);
    }

    #[test]
    fn test_cluster_methods() {
        let c0 = Cluster::new(0);
        let c1 = Cluster::new(1);
        let c2 = Cluster::new(2); // First valid data cluster
        let c100 = Cluster::new(100); // Valid data cluster

        // Test reserved/system clusters
        assert!(!c0.is_valid(FatType::Fat12));
        assert!(!c0.is_valid(FatType::Fat16));
        assert!(!c0.is_valid(FatType::Fat32));

        assert!(!c1.is_valid(FatType::Fat12));
        assert!(!c1.is_valid(FatType::Fat16));
        assert!(!c1.is_valid(FatType::Fat32));

        // Test valid data clusters
        assert!(c2.is_valid(FatType::Fat12));
        assert!(c2.is_valid(FatType::Fat16));
        assert!(c2.is_valid(FatType::Fat32));

        assert!(c100.is_valid(FatType::Fat12));
        assert!(c100.is_valid(FatType::Fat16));
        assert!(c100.is_valid(FatType::Fat32));

        // Test cluster type identification for FAT12
        let eoc_fat12 = Cluster::new(0xFF8);
        let bad_fat12 = Cluster::new(0xFF7);
        let res_fat12 = Cluster::new(0xFF6);

        assert!(eoc_fat12.is_end_of_chain(FatType::Fat12));
        assert!(bad_fat12.is_bad(FatType::Fat12));
        assert!(res_fat12.is_reserved(FatType::Fat12));

        // Test cluster type identification for FAT16
        let eoc_fat16 = Cluster::new(0xFFF8);
        let bad_fat16 = Cluster::new(0xFFF7);
        let res_fat16 = Cluster::new(0xFFF6);

        assert!(eoc_fat16.is_end_of_chain(FatType::Fat16));
        assert!(bad_fat16.is_bad(FatType::Fat16));
        assert!(res_fat16.is_reserved(FatType::Fat16));

        // Test cluster type identification for FAT32
        let eoc_fat32 = Cluster::new(0x0FFF_FFF8);
        let bad_fat32 = Cluster::new(0x0FFF_FFF7);
        let res_fat32 = Cluster::new(0x0FFF_FFF6);

        assert!(eoc_fat32.is_end_of_chain(FatType::Fat32));
        assert!(bad_fat32.is_bad(FatType::Fat32));
        assert!(res_fat32.is_reserved(FatType::Fat32));

        // Test the free cluster method
        let free = Cluster::new(0);
        assert!(free.is_free());
        assert!(!c2.is_free());
    }
}
