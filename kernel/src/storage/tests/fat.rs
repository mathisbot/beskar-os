#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::mem::size_of;

use storage::fs::fat::{
    Cluster, FatError, FatResult, FatType,
    bs::{BootSector, ExtendedBootSector},
    fat::{FatEntries, FatEntry},
    file::FatFile,
};

/// A mock storage volume for testing the FAT filesystem
struct MockVolume {
    // Raw storage
    data: Vec<u8>,
    // FAT type (12, 16, or 32)
    fat_type: FatType,
    // Number of bytes per sector
    bytes_per_sector: u16,
    // Number of sectors per cluster
    sectors_per_cluster: u8,
    // First data sector
    first_data_sector: u32,
    // Root directory location
    root_dir_pos: u32,
}

impl MockVolume {
    fn new_fat12(size_mb: u64) -> Self {
        let mut volume = Self {
            data: alloc::vec![0; (size_mb * 1024 * 1024) as usize],
            fat_type: FatType::Fat12,
            bytes_per_sector: 512,
            sectors_per_cluster: 1,
            first_data_sector: 0,
            root_dir_pos: 0,
        };
        volume.initialize_fat12();
        volume
    }

    fn new_fat16(size_mb: u64) -> Self {
        let mut volume = Self {
            data: alloc::vec![0; (size_mb * 1024 * 1024) as usize],
            fat_type: FatType::Fat16,
            bytes_per_sector: 512,
            sectors_per_cluster: 4,
            first_data_sector: 0,
            root_dir_pos: 0,
        };
        volume.initialize_fat16();
        volume
    }

    fn new_fat32(size_mb: u64) -> Self {
        let mut volume = Self {
            data: alloc::vec![0; (size_mb * 1024 * 1024) as usize],
            fat_type: FatType::Fat32,
            bytes_per_sector: 512,
            sectors_per_cluster: 8,
            first_data_sector: 0,
            root_dir_pos: 0,
        };
        volume.initialize_fat32();
        volume
    }

    fn initialize_fat12(&mut self) {
        // Create boot sector
        let boot_sector = BootSector::new_fat12()
            .with_bytes_per_sector(self.bytes_per_sector)
            .with_sectors_per_cluster(self.sectors_per_cluster)
            .with_reserved_sectors(1)
            .with_fat_count(2)
            .with_root_entries(224)
            .with_media_descriptor(0xF8);

        // Write boot sector
        let boot_sector_bytes = unsafe {
            core::slice::from_raw_parts((&raw const boot_sector).cast(), size_of::<BootSector>())
        };
        self.data[..boot_sector_bytes.len()].copy_from_slice(boot_sector_bytes);

        // Calculate key offsets
        let reserved_sectors = 1u32;
        let fat_sectors = u32::from(boot_sector.bpb().sectors_per_fat());
        let root_dir_sectors = 224 * 32 / u32::from(self.bytes_per_sector);

        self.root_dir_pos = reserved_sectors * u32::from(self.bytes_per_sector);
        self.first_data_sector = (reserved_sectors + fat_sectors * 2 + root_dir_sectors)
            * u32::from(self.bytes_per_sector);
    }

    fn initialize_fat16(&mut self) {
        // Create boot sector
        let boot_sector = BootSector::new_fat16()
            .with_bytes_per_sector(self.bytes_per_sector)
            .with_sectors_per_cluster(self.sectors_per_cluster)
            .with_reserved_sectors(1)
            .with_fat_count(2)
            .with_root_entries(512)
            .with_media_descriptor(0xF8);

        // Write boot sector
        let boot_sector_bytes = unsafe {
            core::slice::from_raw_parts((&raw const boot_sector).cast(), size_of::<BootSector>())
        };
        self.data[..boot_sector_bytes.len()].copy_from_slice(boot_sector_bytes);

        // Calculate key offsets
        let reserved_sectors = 1u32;
        let fat_sectors = u32::from(boot_sector.bpb().sectors_per_fat());
        let root_dir_sectors = 512 * 32 / u32::from(self.bytes_per_sector);

        self.root_dir_pos = reserved_sectors * u32::from(self.bytes_per_sector);
        self.first_data_sector = (reserved_sectors + fat_sectors * 2 + root_dir_sectors)
            * u32::from(self.bytes_per_sector);
    }

    fn initialize_fat32(&mut self) {
        // Create boot sector
        let boot_sector = ExtendedBootSector::new()
            .with_bytes_per_sector(self.bytes_per_sector)
            .with_sectors_per_cluster(self.sectors_per_cluster)
            .with_reserved_sectors(32)
            .with_fat_count(2)
            .with_media_descriptor(0xF8) // Fixed disk
            .with_root_cluster(2)
            .with_volume_id(0x12345678);

        // Write boot sector
        let boot_sector_bytes = unsafe {
            core::slice::from_raw_parts(
                (&raw const boot_sector).cast(),
                size_of::<ExtendedBootSector>(),
            )
        };
        self.data[..boot_sector_bytes.len()].copy_from_slice(boot_sector_bytes);

        // Calculate key offsets
        let reserved_sectors = 32u32;
        let fat_sectors = boot_sector.bpb().sectors_per_fat();

        self.root_dir_pos = 2; // Cluster number for root directory
        self.first_data_sector =
            (reserved_sectors + fat_sectors * 2) * u32::from(self.bytes_per_sector);
    }

    fn bytes_per_cluster(&self) -> u32 {
        u32::from(self.bytes_per_sector) * u32::from(self.sectors_per_cluster)
    }

    fn cluster_offset(&self, cluster: Cluster) -> usize {
        ((cluster.value() - 2) * self.bytes_per_cluster() + self.first_data_sector) as usize
    }

    fn read_cluster(&self, cluster: Cluster, offset: u32, buffer: &mut [u8]) -> FatResult<()> {
        if !cluster.is_valid(self.fat_type) {
            return Err(FatError::InvalidCluster);
        }

        let cluster_offset = self.cluster_offset(cluster);
        let data_offset = cluster_offset + offset as usize;

        if offset >= self.bytes_per_cluster() {
            return Err(FatError::OutOfBounds);
        }

        let bytes_to_read = buffer
            .len()
            .min((self.bytes_per_cluster() - offset) as usize);
        buffer[..bytes_to_read]
            .copy_from_slice(&self.data[data_offset..data_offset + bytes_to_read]);

        Ok(())
    }

    fn write_cluster(&mut self, cluster: Cluster, offset: u32, data: &[u8]) -> FatResult<()> {
        if !cluster.is_valid(self.fat_type) {
            return Err(FatError::InvalidCluster);
        }

        let cluster_offset = self.cluster_offset(cluster);
        let data_offset = cluster_offset + offset as usize;

        if offset >= self.bytes_per_cluster() {
            return Err(FatError::OutOfBounds);
        }

        let bytes_to_write = data.len().min((self.bytes_per_cluster() - offset) as usize);
        self.data[data_offset..data_offset + bytes_to_write]
            .copy_from_slice(&data[..bytes_to_write]);

        Ok(())
    }
}

/// A simple in-memory FAT implementation for testing
struct MockFat {
    entries: Vec<u32>,
    fat_type: FatType,
}

impl MockFat {
    fn new(fat_type: FatType, size: usize) -> Self {
        let mut entries = alloc::vec![0; size];

        // Set up reserved entries
        entries[0] = match fat_type {
            FatType::Fat12 => 0xFF8,
            FatType::Fat16 => 0xFFF8,
            FatType::Fat32 => 0x0FFFFFF8,
        };

        entries[1] = match fat_type {
            FatType::Fat12 => 0xFFF,
            FatType::Fat16 => 0xFFFF,
            FatType::Fat32 => 0x0FFFFFFF,
        };

        Self { entries, fat_type }
    }
}

impl FatEntries for MockFat {
    fn fat_type(&self) -> FatType {
        self.fat_type
    }

    fn get(&self, cluster: Cluster) -> FatResult<FatEntry> {
        let idx = cluster.value() as usize;

        if idx >= self.entries.len() {
            return Err(FatError::OutOfBounds);
        }

        let value = self.entries[idx];

        if value == 0 {
            return Ok(FatEntry::Free);
        }

        if cluster.is_reserved(self.fat_type) {
            return Ok(FatEntry::Reserved);
        }

        if cluster.is_bad(self.fat_type) {
            return Ok(FatEntry::Bad);
        }

        if Cluster::new(value).is_end_of_chain(self.fat_type) {
            return Ok(FatEntry::EndOfChain);
        }

        Ok(FatEntry::Next(Cluster::new(value)))
    }

    fn set(&mut self, cluster: Cluster, entry: FatEntry) -> FatResult<()> {
        let idx = cluster.value() as usize;

        if idx >= self.entries.len() {
            return Err(FatError::OutOfBounds);
        }

        self.entries[idx] = match entry {
            FatEntry::Free => 0,
            FatEntry::Reserved => match self.fat_type {
                FatType::Fat12 => 0xFF0,
                FatType::Fat16 => 0xFFF0,
                FatType::Fat32 => 0x0FFFFFF0,
            },
            FatEntry::Bad => match self.fat_type {
                FatType::Fat12 => 0xFF7,
                FatType::Fat16 => 0xFFF7,
                FatType::Fat32 => 0x0FFFFFF7,
            },
            FatEntry::EndOfChain => match self.fat_type {
                FatType::Fat12 => 0xFFF,
                FatType::Fat16 => 0xFFFF,
                FatType::Fat32 => 0x0FFFFFFF,
            },
            FatEntry::Next(next) => next.value(),
        };

        Ok(())
    }

    fn alloc_cluster(&mut self) -> FatResult<Cluster> {
        for (idx, &value) in self.entries.iter().enumerate().skip(2) {
            if value == 0 {
                // Found a free cluster
                let cluster = Cluster::new(idx as u32);
                self.set(cluster, FatEntry::EndOfChain)?;
                return Ok(cluster);
            }
        }

        Err(FatError::OutOfBounds)
    }

    fn alloc_cluster_chain(&mut self, count: usize) -> FatResult<Cluster> {
        if count == 0 {
            return Err(FatError::InvalidParameter);
        }

        // Find enough free clusters
        let free_count = self.entries.iter().skip(2).filter(|&&v| v == 0).count();
        if free_count < count {
            return Err(FatError::OutOfBounds);
        }

        let first_cluster = self.alloc_cluster()?;
        let mut current = first_cluster;

        // Allocate and link remaining clusters
        for _ in 1..count {
            let next_cluster = self.alloc_cluster()?;
            self.set(current, FatEntry::Next(next_cluster))?;
            current = next_cluster;
        }

        Ok(first_cluster)
    }

    fn free_cluster(&mut self, cluster: Cluster) -> FatResult<()> {
        if !cluster.is_valid(self.fat_type) {
            return Err(FatError::InvalidCluster);
        }

        self.set(cluster, FatEntry::Free)
    }

    fn free_cluster_chain(&mut self, start_cluster: Cluster) -> FatResult<()> {
        let mut current = start_cluster;

        while !current.is_free() && current.is_valid(self.fat_type) {
            match self.get(current)? {
                FatEntry::Next(next) => {
                    self.set(current, FatEntry::Free)?;
                    current = next;
                }
                FatEntry::EndOfChain => {
                    self.set(current, FatEntry::Free)?;
                    break;
                }
                _ => return Err(FatError::InvalidCluster),
            }
        }

        Ok(())
    }

    fn count_free(&self) -> FatResult<u32> {
        let count = self.entries.iter().skip(2).filter(|&&v| v == 0).count();
        Ok(count as u32)
    }
}

#[test]
fn test_fat12_basic_file_operations() {
    // Create a volume and FAT
    let mut volume = MockVolume::new_fat12(1); // 1MB volume
    let mut fat = MockFat::new(FatType::Fat12, 4096);
    let bytes_per_cluster = volume.bytes_per_cluster();

    // Create a simple file
    let test_data = b"Hello, FAT12 filesystem!";

    // Create an empty file
    let mut file = FatFile::new(
        &mut fat,
        Cluster::new(0), // No initial cluster
        0,               // Empty file
        bytes_per_cluster,
    )
    .expect("Failed to create file");

    // Write data to file
    let bytes_written = file
        .write(test_data, &mut |cluster, offset, data| {
            volume.write_cluster(cluster, offset, data)
        })
        .expect("Failed to write data");

    assert_eq!(bytes_written, test_data.len());
    assert_eq!(file.size(), test_data.len() as u64);

    // Read data back
    let mut read_buffer = [0u8; 32];
    file.seek(0).expect("Failed to seek to beginning");

    let bytes_read = file
        .read(&mut read_buffer, &mut |cluster, offset, buffer| {
            volume.read_cluster(cluster, offset, buffer)
        })
        .expect("Failed to read data");

    assert_eq!(bytes_read, test_data.len());
    assert_eq!(&read_buffer[..bytes_read], test_data);

    // Test truncation
    file.truncate(10).expect("Failed to truncate file");
    assert_eq!(file.size(), 10);

    // Verify truncated content
    let mut truncated_buffer = [0u8; 10];
    file.seek(0).expect("Failed to seek to beginning");

    let truncated_read = file
        .read(&mut truncated_buffer, &mut |cluster, offset, buffer| {
            volume.read_cluster(cluster, offset, buffer)
        })
        .expect("Failed to read truncated data");

    assert_eq!(truncated_read, 10);
    assert_eq!(&truncated_buffer, &test_data[..10]);
}

#[test]
fn test_fat16_multi_cluster_file() {
    // Create a volume and FAT
    let mut volume = MockVolume::new_fat16(8); // 8MB volume
    let mut fat = MockFat::new(FatType::Fat16, 65536);
    let bytes_per_cluster = volume.bytes_per_cluster();

    // Create a file large enough to span multiple clusters
    let large_data = alloc::vec![0x41u8; 10000]; // 10KB of 'A's

    // Create an empty file
    let mut file = FatFile::new(
        &mut fat,
        Cluster::new(0), // No initial cluster
        0,               // Empty file
        bytes_per_cluster,
    )
    .expect("Failed to create file");

    // Write large data to file (will span clusters)
    let bytes_written = file
        .write(&large_data, &mut |cluster, offset, data| {
            volume.write_cluster(cluster, offset, data)
        })
        .expect("Failed to write data");

    assert_eq!(bytes_written, large_data.len());
    assert_eq!(file.size(), large_data.len() as u64);

    // Test random access - read from the middle
    file.seek(5000).expect("Failed to seek to middle");

    let mut mid_buffer = [0u8; 1000];
    let mid_read = file
        .read(&mut mid_buffer, &mut |cluster, offset, buffer| {
            volume.read_cluster(cluster, offset, buffer)
        })
        .expect("Failed to read from middle");

    assert_eq!(mid_read, 1000);
    assert_eq!(&mid_buffer, &large_data[5000..6000]);
}

#[test]
fn test_fat32_large_file_operations() {
    // Create a volume and FAT
    let mut volume = MockVolume::new_fat32(64); // 64MB volume
    let mut fat = MockFat::new(FatType::Fat32, 65536);
    let bytes_per_cluster = volume.bytes_per_cluster();

    // Test cluster allocation and chaining
    let cluster1 = fat.alloc_cluster().expect("Failed to allocate cluster");
    let cluster2 = fat.alloc_cluster().expect("Failed to allocate cluster");

    assert!(cluster1.is_valid(FatType::Fat32));
    assert!(cluster2.is_valid(FatType::Fat32));
    assert_ne!(cluster1.value(), cluster2.value());

    // Test allocating a chain of clusters
    let chain_start = fat
        .alloc_cluster_chain(5)
        .expect("Failed to allocate chain");
    assert!(chain_start.is_valid(FatType::Fat32));

    // Verify chain links are correct
    let mut current = chain_start;
    let mut count = 0;

    while let FatEntry::Next(next) = fat.get(current).expect("Failed to get entry") {
        current = next;
        count += 1;
        if count >= 5 {
            panic!("Chain too long or cyclical");
        }
    }

    // The last cluster should be an end of chain marker
    assert!(matches!(
        fat.get(current).expect("Failed to get entry"),
        FatEntry::EndOfChain
    ));
    assert_eq!(count, 4); // We should have traversed 4 Next entries (5 clusters total)

    // Free the cluster chain and verify it's properly freed
    fat.free_cluster_chain(chain_start)
        .expect("Failed to free chain");
    assert!(matches!(
        fat.get(chain_start).expect("Failed to get entry"),
        FatEntry::Free
    ));

    // Create and test a large file
    let mut file = FatFile::new(
        &mut fat,
        Cluster::new(0), // No initial cluster
        0,               // Empty file
        bytes_per_cluster,
    )
    .expect("Failed to create file");

    // Create a 40KB file
    let very_large_data = alloc::vec!['B' as u8; 40000]; // 40KB of 'B's

    let bytes_written = file
        .write(&very_large_data, &mut |cluster, offset, data| {
            volume.write_cluster(cluster, offset, data)
        })
        .expect("Failed to write data");

    assert_eq!(bytes_written, very_large_data.len());
    assert_eq!(file.size(), very_large_data.len() as u64);

    // Verify file content by reading it back in chunks
    file.seek(0).expect("Failed to seek to beginning");

    let mut full_buffer = alloc::vec![0u8; very_large_data.len()];
    let mut total_read = 0;
    let mut buffer_pos = 0;

    while total_read < very_large_data.len() {
        let read_size = file
            .read(
                &mut full_buffer[buffer_pos..],
                &mut |cluster, offset, buffer| volume.read_cluster(cluster, offset, buffer),
            )
            .expect("Failed to read data chunk");

        if read_size == 0 {
            break;
        }

        total_read += read_size;
        buffer_pos += read_size;
    }

    assert_eq!(total_read, very_large_data.len());
    assert_eq!(&full_buffer, &very_large_data);
}
