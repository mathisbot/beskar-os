use crate::fs::fat::{Cluster, FatError, FatResult, FatType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// FAT12/16/32 table entry
pub enum FatEntry {
    /// Free cluster
    Free,
    /// Used cluster, pointing to the next cluster in the chain
    Next(Cluster),
    /// Last cluster in the chain
    EndOfChain,
    /// Bad cluster
    Bad,
    /// Reserved cluster
    Reserved,
}

/// Collection of FAT entries
pub trait FatEntries {
    #[must_use]
    /// Returns the type of FAT (FAT12, FAT16, FAT32)
    fn fat_type(&self) -> FatType;

    /// Returns the entry value for the given cluster
    fn get(&self, cluster: Cluster) -> FatResult<FatEntry>;

    /// Sets the entry value for the given cluster
    fn set(&mut self, cluster: Cluster, entry: FatEntry) -> FatResult<()>;

    #[must_use]
    /// Returns an iterator over all clusters in a chain starting from the given cluster
    fn chain_iter(&self, start: Cluster) -> FatChainIter<'_, Self>
    where
        Self: Sized,
    {
        FatChainIter {
            fat: self,
            next: Some(start),
        }
    }

    /// Allocates a new cluster and returns its number
    fn alloc_cluster(&mut self) -> FatResult<Cluster>;

    /// Allocates a chain of clusters and returns the first cluster number
    fn alloc_cluster_chain(&mut self, count: usize) -> FatResult<Cluster>;

    /// Frees a cluster
    fn free_cluster(&mut self, cluster: Cluster) -> FatResult<()>;

    /// Frees a chain of clusters starting from the given cluster
    fn free_cluster_chain(&mut self, start: Cluster) -> FatResult<()>;

    /// Counts the number of free clusters
    fn count_free(&self) -> FatResult<u32>;
}

/// Iterator over a chain of clusters
pub struct FatChainIter<'a, T: FatEntries> {
    fat: &'a T,
    next: Option<Cluster>,
}

impl<T: FatEntries> Iterator for FatChainIter<'_, T> {
    type Item = Cluster;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;

        // Get the next cluster in the chain
        match self.fat.get(current) {
            Ok(FatEntry::Next(next)) => {
                self.next = Some(next);
            }
            _ => {
                self.next = None;
            }
        }

        Some(current)
    }
}

/// FAT12 entry handling
pub(crate) mod fat12 {
    use super::{Cluster, FatEntry, FatError, FatResult};

    pub fn read_fat_entry(fat: &[u8], cluster: Cluster) -> FatResult<FatEntry> {
        let cluster_val = usize::try_from(cluster.value()).unwrap();
        let offset = cluster_val + (cluster_val / 2); // 3 bytes per 2 entries

        if offset + 1 >= fat.len() {
            return Err(FatError::OutOfBounds);
        }

        let mut value = u16::from(fat[offset]);
        value |= u16::from(fat[offset + 1]) << 8;

        // For odd cluster numbers, take the high 12 bits
        if cluster_val & 1 != 0 {
            value >>= 4;
        } else {
            // For even cluster numbers, take the low 12 bits
            value &= 0x0FFF;
        }

        match value {
            0 => Ok(FatEntry::Free),
            0x0FF7 => Ok(FatEntry::Bad),
            0x0FF0..=0x0FF6 => Ok(FatEntry::Reserved),
            0x0FF8..=0x0FFF => Ok(FatEntry::EndOfChain),
            val => Ok(FatEntry::Next(Cluster::new(u32::from(val)))),
        }
    }

    pub fn write_fat_entry(fat: &mut [u8], cluster: Cluster, entry: FatEntry) -> FatResult<()> {
        let cluster_val = usize::try_from(cluster.value()).unwrap();
        let offset = cluster_val + (cluster_val / 2); // 3 bytes per 2 entries

        if offset + 1 >= fat.len() {
            return Err(FatError::OutOfBounds);
        }

        // Convert the entry to a raw value
        let value = match entry {
            FatEntry::Free => 0,
            FatEntry::Next(next) => u16::try_from(next.value() & 0x0FFF).unwrap(),
            FatEntry::EndOfChain => 0x0FFF,
            FatEntry::Bad => 0x0FF7,
            FatEntry::Reserved => 0x0FF6,
        };

        // Read the original bytes
        let bytes = [fat[offset], fat[offset + 1]];
        let word = u16::from_le_bytes(bytes);

        let new_word = if cluster_val & 1 != 0 {
            // Odd cluster: modify the high 12 bits
            (word & 0x000F) | (value << 4)
        } else {
            // Even cluster: modify the low 12 bits
            (word & 0xF000) | value
        };

        // Write the modified bytes back
        let new_bytes = new_word.to_le_bytes();
        fat[offset] = new_bytes[0];
        fat[offset + 1] = new_bytes[1];

        Ok(())
    }
}

/// FAT16 entry handling
pub(crate) mod fat16 {
    use super::{Cluster, FatEntry, FatError, FatResult};

    pub fn read_fat_entry(fat: &[u8], cluster: Cluster) -> FatResult<FatEntry> {
        let offset = cluster.value() as usize * 2;

        if offset + 1 >= fat.len() {
            return Err(FatError::OutOfBounds);
        }

        let value = u16::from_le_bytes([fat[offset], fat[offset + 1]]);

        match value {
            0 => Ok(FatEntry::Free),
            0xFFF7 => Ok(FatEntry::Bad),
            0xFFF0..=0xFFF6 => Ok(FatEntry::Reserved),
            0xFFF8..=0xFFFF => Ok(FatEntry::EndOfChain),
            val => Ok(FatEntry::Next(Cluster::new(u32::from(val)))),
        }
    }

    pub fn write_fat_entry(fat: &mut [u8], cluster: Cluster, entry: FatEntry) -> FatResult<()> {
        let offset = cluster.value() as usize * 2;

        if offset + 1 >= fat.len() {
            return Err(FatError::OutOfBounds);
        }

        // Convert the entry to a raw value
        let value = match entry {
            FatEntry::Free => 0,
            FatEntry::Next(next) => u16::try_from(next.value() & 0xFFFF).unwrap(),
            FatEntry::EndOfChain => 0xFFFF,
            FatEntry::Bad => 0xFFF7,
            FatEntry::Reserved => 0xFFF6,
        };

        // Write the bytes
        let bytes = value.to_le_bytes();
        fat[offset] = bytes[0];
        fat[offset + 1] = bytes[1];

        Ok(())
    }
}

/// FAT32 entry handling
pub(crate) mod fat32 {
    use super::{Cluster, FatEntry, FatError, FatResult};

    pub fn read_fat_entry(fat: &[u8], cluster: Cluster) -> FatResult<FatEntry> {
        let offset = cluster.value() as usize * 4;

        if offset + 3 >= fat.len() {
            return Err(FatError::OutOfBounds);
        }

        // Read 4 bytes but only use the lower 28 bits
        let value = u32::from_le_bytes([
            fat[offset],
            fat[offset + 1],
            fat[offset + 2],
            fat[offset + 3] & 0x0F,
        ]);

        match value {
            0 => Ok(FatEntry::Free),
            0x0FFF_FFF7 => Ok(FatEntry::Bad),
            0x0FFF_FFF0..=0x0FFF_FFF6 => Ok(FatEntry::Reserved),
            0x0FFF_FFF8..=0x0FFF_FFFF => Ok(FatEntry::EndOfChain),
            val => Ok(FatEntry::Next(Cluster::new(val))),
        }
    }

    pub fn write_fat_entry(fat: &mut [u8], cluster: Cluster, entry: FatEntry) -> FatResult<()> {
        let offset = cluster.value() as usize * 4;

        if offset + 3 >= fat.len() {
            return Err(FatError::OutOfBounds);
        }

        // Convert the entry to a raw value (only the lower 28 bits are used)
        let value = match entry {
            FatEntry::Free => 0,
            FatEntry::Next(next) => next.value() & 0x0FFF_FFFF,
            FatEntry::EndOfChain => 0x0FFF_FFFF,
            FatEntry::Bad => 0x0FFF_FFF7,
            FatEntry::Reserved => 0x0FFF_FFF6,
        };

        // Preserve the high 4 bits of the last byte
        let last_byte = fat[offset + 3] & 0xF0 | ((value >> 24) & 0x0F) as u8;

        // Write the bytes
        fat[offset] = (value & 0xFF) as u8;
        fat[offset + 1] = ((value >> 8) & 0xFF) as u8;
        fat[offset + 2] = ((value >> 16) & 0xFF) as u8;
        fat[offset + 3] = last_byte;

        Ok(())
    }
}

/// In-memory FAT table implementation
pub struct FatTable<'a> {
    fat_type: FatType,
    data: &'a mut [u8],
    free_count: u32,
}

impl<'a> FatTable<'a> {
    #[must_use]
    #[inline]
    /// Creates a new FAT table wrapper
    pub fn new(fat_type: FatType, data: &'a mut [u8]) -> Self {
        let mut table = Self {
            fat_type,
            data,
            free_count: 0,
        };

        // Count free clusters
        table.free_count = table.count_free().unwrap_or(0);

        table
    }

    #[must_use]
    #[inline]
    /// Returns the maximum number of clusters for this FAT type
    const fn max_clusters(&self) -> u32 {
        match self.fat_type {
            FatType::Fat12 => 0x0FF6,
            FatType::Fat16 => 0xFFF6,
            FatType::Fat32 => 0x0FFF_FFF6,
        }
    }
}

impl FatEntries for FatTable<'_> {
    #[inline]
    fn fat_type(&self) -> FatType {
        self.fat_type
    }

    fn get(&self, cluster: Cluster) -> FatResult<FatEntry> {
        if !cluster.is_valid(self.fat_type) {
            return Err(FatError::InvalidCluster);
        }

        match self.fat_type {
            FatType::Fat12 => fat12::read_fat_entry(self.data, cluster),
            FatType::Fat16 => fat16::read_fat_entry(self.data, cluster),
            FatType::Fat32 => fat32::read_fat_entry(self.data, cluster),
        }
    }

    fn set(&mut self, cluster: Cluster, entry: FatEntry) -> FatResult<()> {
        if !cluster.is_valid(self.fat_type) && cluster.value() != 0 && cluster.value() != 1 {
            return Err(FatError::InvalidCluster);
        }

        // Update free count if changing from/to free
        let old_entry = self.get(cluster)?;
        if old_entry == FatEntry::Free && entry != FatEntry::Free {
            self.free_count = self.free_count.saturating_sub(1);
        } else if old_entry != FatEntry::Free && entry == FatEntry::Free {
            self.free_count = self.free_count.saturating_add(1);
        }

        match self.fat_type {
            FatType::Fat12 => fat12::write_fat_entry(self.data, cluster, entry),
            FatType::Fat16 => fat16::write_fat_entry(self.data, cluster, entry),
            FatType::Fat32 => fat32::write_fat_entry(self.data, cluster, entry),
        }
    }

    fn alloc_cluster(&mut self) -> FatResult<Cluster> {
        if self.free_count == 0 {
            return Err(FatError::OutOfBounds);
        }

        // Start searching from cluster 2 (the first valid data cluster)
        for i in 2..=self.max_clusters() {
            let cluster = Cluster::new(i);
            match self.get(cluster) {
                Ok(FatEntry::Free) => {
                    // Mark as end of chain and return it
                    self.set(cluster, FatEntry::EndOfChain)?;
                    return Ok(cluster);
                }
                Err(e) => return Err(e),
                _ => {}
            }
        }

        Err(FatError::OutOfBounds)
    }

    fn alloc_cluster_chain(&mut self, count: usize) -> FatResult<Cluster> {
        if count == 0 {
            return Err(FatError::InvalidParameter);
        }

        if usize::try_from(self.free_count).unwrap() < count {
            return Err(FatError::OutOfBounds);
        }

        // Allocate the first cluster
        let first = self.alloc_cluster()?;
        let mut prev = first;

        // Allocate additional clusters
        for _ in 1..count {
            let next = self.alloc_cluster()?;
            // Link the previous cluster to this one
            self.set(prev, FatEntry::Next(next))?;
            prev = next;
        }

        Ok(first)
    }

    fn free_cluster(&mut self, cluster: Cluster) -> FatResult<()> {
        if !cluster.is_valid(self.fat_type) {
            return Err(FatError::InvalidCluster);
        }

        // Mark the cluster as free
        self.set(cluster, FatEntry::Free)
    }

    fn free_cluster_chain(&mut self, start: Cluster) -> FatResult<()> {
        if !start.is_valid(self.fat_type) {
            return Err(FatError::InvalidCluster);
        }

        let mut current = start;

        while !current.is_free() {
            match self.get(current)? {
                FatEntry::Next(next) => {
                    // Mark the current cluster as free
                    self.set(current, FatEntry::Free)?;
                    current = next;
                }
                FatEntry::EndOfChain => {
                    // Mark the last cluster as free and exit
                    self.set(current, FatEntry::Free)?;
                    break;
                }
                _ => return Err(FatError::InvalidCluster),
            }
        }

        Ok(())
    }

    fn count_free(&self) -> FatResult<u32> {
        // Start searching from cluster 2 (the first valid data cluster)
        let mut count = 0;
        for i in 2..=self.max_clusters() {
            let cluster = Cluster::new(i);
            match self.get(cluster) {
                Ok(FatEntry::Free) => count += 1,
                Err(FatError::OutOfBounds) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fat12_read_write() {
        // Create a fake FAT12 table
        let mut fat = [0u8; 512]; // Small test FAT

        // Test writing entries
        assert!(fat12::write_fat_entry(&mut fat, Cluster::new(2), FatEntry::EndOfChain).is_ok());
        assert!(fat12::write_fat_entry(&mut fat, Cluster::new(3), FatEntry::Bad).is_ok());
        assert!(
            fat12::write_fat_entry(&mut fat, Cluster::new(4), FatEntry::Next(Cluster::new(5)))
                .is_ok()
        );

        // Test reading entries
        assert_eq!(
            fat12::read_fat_entry(&fat, Cluster::new(2)).unwrap(),
            FatEntry::EndOfChain
        );
        assert_eq!(
            fat12::read_fat_entry(&fat, Cluster::new(3)).unwrap(),
            FatEntry::Bad
        );
        assert_eq!(
            fat12::read_fat_entry(&fat, Cluster::new(4)).unwrap(),
            FatEntry::Next(Cluster::new(5))
        );

        // Test special cases - free cluster
        assert!(fat12::write_fat_entry(&mut fat, Cluster::new(6), FatEntry::Free).is_ok());
        assert_eq!(
            fat12::read_fat_entry(&fat, Cluster::new(6)).unwrap(),
            FatEntry::Free
        );

        // Test reserved cluster
        assert!(fat12::write_fat_entry(&mut fat, Cluster::new(7), FatEntry::Reserved).is_ok());
        assert_eq!(
            fat12::read_fat_entry(&fat, Cluster::new(7)).unwrap(),
            FatEntry::Reserved
        );

        // Test edge of FAT
        assert_eq!(
            fat12::read_fat_entry(&fat, Cluster::new(350)).unwrap_err(),
            FatError::OutOfBounds
        );
    }

    #[test]
    fn test_fat16_read_write() {
        // Create a fake FAT16 table
        let mut fat = [0u8; 512]; // Small test FAT

        // Test writing entries
        assert!(fat16::write_fat_entry(&mut fat, Cluster::new(2), FatEntry::EndOfChain).is_ok());
        assert!(fat16::write_fat_entry(&mut fat, Cluster::new(3), FatEntry::Bad).is_ok());
        assert!(
            fat16::write_fat_entry(&mut fat, Cluster::new(4), FatEntry::Next(Cluster::new(5)))
                .is_ok()
        );

        // Test reading entries
        assert_eq!(
            fat16::read_fat_entry(&fat, Cluster::new(2)).unwrap(),
            FatEntry::EndOfChain
        );
        assert_eq!(
            fat16::read_fat_entry(&fat, Cluster::new(3)).unwrap(),
            FatEntry::Bad
        );
        assert_eq!(
            fat16::read_fat_entry(&fat, Cluster::new(4)).unwrap(),
            FatEntry::Next(Cluster::new(5))
        );

        // Test special cases - free cluster
        assert!(fat16::write_fat_entry(&mut fat, Cluster::new(6), FatEntry::Free).is_ok());
        assert_eq!(
            fat16::read_fat_entry(&fat, Cluster::new(6)).unwrap(),
            FatEntry::Free
        );

        // Test reserved cluster
        assert!(fat16::write_fat_entry(&mut fat, Cluster::new(7), FatEntry::Reserved).is_ok());
        assert_eq!(
            fat16::read_fat_entry(&fat, Cluster::new(7)).unwrap(),
            FatEntry::Reserved
        );

        // Test edge of FAT
        assert_eq!(
            fat16::read_fat_entry(&fat, Cluster::new(256)).unwrap_err(),
            FatError::OutOfBounds
        );
    }

    #[test]
    fn test_fat32_read_write() {
        // Create a fake FAT32 table
        let mut fat = [0u8; 512]; // Small test FAT

        // Test writing entries
        assert!(fat32::write_fat_entry(&mut fat, Cluster::new(2), FatEntry::EndOfChain).is_ok());
        assert!(fat32::write_fat_entry(&mut fat, Cluster::new(3), FatEntry::Bad).is_ok());
        assert!(
            fat32::write_fat_entry(&mut fat, Cluster::new(4), FatEntry::Next(Cluster::new(5)))
                .is_ok()
        );

        // Test reading entries
        assert_eq!(
            fat32::read_fat_entry(&fat, Cluster::new(2)).unwrap(),
            FatEntry::EndOfChain
        );
        assert_eq!(
            fat32::read_fat_entry(&fat, Cluster::new(3)).unwrap(),
            FatEntry::Bad
        );
        assert_eq!(
            fat32::read_fat_entry(&fat, Cluster::new(4)).unwrap(),
            FatEntry::Next(Cluster::new(5))
        );

        // Test special cases - free cluster
        assert!(fat32::write_fat_entry(&mut fat, Cluster::new(6), FatEntry::Free).is_ok());
        assert_eq!(
            fat32::read_fat_entry(&fat, Cluster::new(6)).unwrap(),
            FatEntry::Free
        );

        // Test reserved cluster
        assert!(fat32::write_fat_entry(&mut fat, Cluster::new(7), FatEntry::Reserved).is_ok());
        assert_eq!(
            fat32::read_fat_entry(&fat, Cluster::new(7)).unwrap(),
            FatEntry::Reserved
        );

        // Test edge of FAT
        assert_eq!(
            fat32::read_fat_entry(&fat, Cluster::new(128)).unwrap_err(),
            FatError::OutOfBounds
        );
    }

    #[test]
    fn test_fat_table() {
        // Create a FAT16 table
        let mut data = vec![0u8; 512];
        let mut table = FatTable::new(FatType::Fat16, &mut data);

        // Allocate clusters
        let first = table.alloc_cluster().unwrap();
        let second = table.alloc_cluster().unwrap();
        let third = table.alloc_cluster().unwrap();

        assert_eq!(first, Cluster::new(2)); // First data cluster is 2
        assert_eq!(second, Cluster::new(3));
        assert_eq!(third, Cluster::new(4));

        // Link clusters
        assert!(table.set(first, FatEntry::Next(second)).is_ok());
        assert!(table.set(second, FatEntry::Next(third)).is_ok());
        assert!(table.set(third, FatEntry::EndOfChain).is_ok());

        // Check chain
        assert_eq!(table.get(first).unwrap(), FatEntry::Next(second));
        assert_eq!(table.get(second).unwrap(), FatEntry::Next(third));
        assert_eq!(table.get(third).unwrap(), FatEntry::EndOfChain);

        // Test iterator
        let mut iter = table.chain_iter(first);
        assert_eq!(iter.next(), Some(first));
        assert_eq!(iter.next(), Some(second));
        assert_eq!(iter.next(), Some(third));
        assert_eq!(iter.next(), None);

        // Test free count
        assert_eq!(table.count_free().unwrap(), table.free_count);

        // Free a cluster
        assert!(table.free_cluster(third).is_ok());
        assert_eq!(table.get(third).unwrap(), FatEntry::Free);

        // Allocate a cluster chain
        let start = table.alloc_cluster_chain(3).unwrap();
        assert_eq!(start, Cluster::new(4)); // Reuse freed clusters first

        // Check chain
        let mut iter = table.chain_iter(start);
        assert_eq!(iter.next(), Some(Cluster::new(4)));
        assert_eq!(iter.next(), Some(Cluster::new(5)));
        assert_eq!(iter.next(), Some(Cluster::new(6)));
        assert_eq!(iter.next(), None);

        // Free a chain
        assert!(table.free_cluster_chain(start).is_ok());

        // Verify all freed
        assert_eq!(table.get(Cluster::new(4)).unwrap(), FatEntry::Free);
        assert_eq!(table.get(Cluster::new(5)).unwrap(), FatEntry::Free);
        assert_eq!(table.get(Cluster::new(6)).unwrap(), FatEntry::Free);
    }

    #[test]
    fn test_cluster_range_checking() {
        let cluster = Cluster::new(10);

        // Valid checks
        assert!(cluster.is_valid(FatType::Fat12));
        assert!(cluster.is_valid(FatType::Fat16));
        assert!(cluster.is_valid(FatType::Fat32));

        // Invalid clusters
        let invalid = Cluster::new(0); // Cluster 0 is invalid
        assert!(!invalid.is_valid(FatType::Fat12));
        assert!(!invalid.is_valid(FatType::Fat16));
        assert!(!invalid.is_valid(FatType::Fat32));

        // Bad cluster markers
        let bad12 = Cluster::new(0xFF7);
        let bad16 = Cluster::new(0xFFF7);
        let bad32 = Cluster::new(0x0FFF_FFF7);

        assert!(bad12.is_bad(FatType::Fat12));
        assert!(!bad12.is_bad(FatType::Fat16));
        assert!(bad16.is_bad(FatType::Fat16));
        assert!(bad32.is_bad(FatType::Fat32));

        // End of chain markers
        let eoc12 = Cluster::new(0xFF8);
        let eoc16 = Cluster::new(0xFFF8);
        let eoc32 = Cluster::new(0x0FFF_FFF8);

        assert!(eoc12.is_end_of_chain(FatType::Fat12));
        assert!(!eoc12.is_end_of_chain(FatType::Fat16));
        assert!(eoc16.is_end_of_chain(FatType::Fat16));
        assert!(eoc32.is_end_of_chain(FatType::Fat32));

        // Free cluster
        let free = Cluster::new(0);
        assert!(free.is_free());
    }

    #[test]
    fn test_error_handling() {
        // Create a small fat table
        let mut data = vec![0u8; 20];
        let mut table = FatTable::new(FatType::Fat16, &mut data);

        // Test out of bounds error
        assert_eq!(
            table.get(Cluster::new(10)).unwrap_err(),
            FatError::OutOfBounds
        );

        // Test invalid cluster error
        assert_eq!(
            table.get(Cluster::new(0)).unwrap_err(),
            FatError::InvalidCluster
        );

        // Test allocation with no space
        for _ in 0..8 {
            // Fill up the small fat
            let _cluster = table.alloc_cluster().unwrap();
        }

        assert_eq!(table.alloc_cluster().unwrap_err(), FatError::OutOfBounds);
        assert_eq!(
            table.alloc_cluster_chain(2).unwrap_err(),
            FatError::OutOfBounds
        );

        // Test invalid parameters
        assert_eq!(
            table.alloc_cluster_chain(0).unwrap_err(),
            FatError::InvalidParameter
        );
    }
}
