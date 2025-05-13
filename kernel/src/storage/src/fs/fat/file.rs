use super::{
    Cluster, FatError, FatResult, RefDataReader, RefDataWriter,
    fat::{FatEntries, FatEntry},
};

/// File abstraction.
pub struct FatFile<'a, T: FatEntries> {
    /// The FAT entries.
    fat: &'a mut T,
    /// First cluster of the file.
    first_cluster: Cluster,
    /// Current position within the file.
    position: u64,
    /// Size of the file in bytes.
    size: u64,
    /// Bytes per cluster.
    bytes_per_cluster: u32,
    /// Current cluster.
    current_cluster: Cluster,
    /// Offset within current cluster.
    cluster_offset: u32,
}

impl<'a, T: FatEntries> FatFile<'a, T> {
    pub fn new(
        fat: &'a mut T,
        first_cluster: Cluster,
        size: u64,
        bytes_per_cluster: u32,
    ) -> FatResult<Self> {
        if !first_cluster.is_valid(fat.fat_type()) && first_cluster.value() != 0 {
            return Err(FatError::InvalidCluster);
        }

        Ok(Self {
            fat,
            first_cluster,
            position: 0,
            size,
            bytes_per_cluster,
            current_cluster: first_cluster,
            cluster_offset: 0,
        })
    }

    #[must_use]
    #[inline]
    /// Returns the current position within the file
    pub const fn position(&self) -> u64 {
        self.position
    }

    #[must_use]
    #[inline]
    /// Returns the size of the file in bytes
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Seeks to a position in the file
    pub fn seek(&mut self, position: u64) -> FatResult<u64> {
        // Ensure we don't seek beyond file size
        let position = position.min(self.size);

        // If moving backwards, start from beginning
        if position < self.position {
            self.current_cluster = self.first_cluster;
            self.cluster_offset = 0;
            self.position = 0;
        }

        // Skip clusters until we reach the target position
        while self.position < position {
            let current_cluster_size = u64::from(self.bytes_per_cluster);
            let remaining_in_cluster = current_cluster_size - u64::from(self.cluster_offset);

            if self.position + remaining_in_cluster <= position {
                // Skip entire cluster
                match self.fat.get(self.current_cluster)? {
                    FatEntry::Next(next) => {
                        self.current_cluster = next;
                        self.cluster_offset = 0;
                        self.position += remaining_in_cluster;
                    }
                    FatEntry::EndOfChain => {
                        // We've reached the end of the chain, so seek to the end of the file
                        self.cluster_offset =
                            u32::try_from(self.size % current_cluster_size).unwrap();
                        self.position = self.size;
                        break;
                    }
                    _ => return Err(FatError::InvalidCluster),
                }
            } else {
                // Partial cluster skip
                let delta = position - self.position;
                self.cluster_offset += u32::try_from(delta).unwrap();
                self.position += delta;
                break;
            }
        }

        Ok(self.position)
    }

    /// Reads data from the file into the provided buffer
    pub fn read(&mut self, buffer: &mut [u8], read_data: RefDataReader) -> FatResult<usize> {
        if self.position >= self.size {
            return Ok(0);
        }

        let bytes_to_read = buffer
            .len()
            .min((self.size - self.position).try_into().unwrap());
        let mut bytes_read = 0;

        while bytes_read < bytes_to_read {
            // Calculate how many bytes we can read from the current cluster
            let cluster_remaining = self.bytes_per_cluster - self.cluster_offset;
            let chunk_size =
                (bytes_to_read - bytes_read).min(cluster_remaining.try_into().unwrap());

            // Read data from the current cluster
            read_data(
                self.current_cluster,
                self.cluster_offset,
                &mut buffer[bytes_read..bytes_read + chunk_size],
            )?;

            bytes_read += chunk_size;
            self.position += u64::try_from(chunk_size).unwrap();
            self.cluster_offset += u32::try_from(chunk_size).unwrap();

            // Move to the next cluster if necessary
            if self.cluster_offset >= self.bytes_per_cluster && bytes_read < bytes_to_read {
                match self.fat.get(self.current_cluster)? {
                    FatEntry::Next(next) => {
                        self.current_cluster = next;
                        self.cluster_offset = 0;
                    }
                    FatEntry::EndOfChain => {
                        // End of file reached
                        break;
                    }
                    _ => return Err(FatError::InvalidCluster),
                }
            }
        }

        Ok(bytes_read)
    }

    /// Writes data to the file from the provided buffer
    pub fn write(&mut self, buffer: &[u8], write_data: RefDataWriter) -> FatResult<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }

        let mut bytes_written = 0;
        let write_size = buffer.len();

        // Ensure we have a valid starting cluster
        if self.first_cluster.value() == 0 {
            // Allocate first cluster
            self.first_cluster = self.fat.alloc_cluster()?;
            self.current_cluster = self.first_cluster;
        }

        // Write data in chunks of cluster_size
        while bytes_written < write_size {
            // Calculate how many bytes we can write to the current cluster
            let cluster_remaining = self.bytes_per_cluster - self.cluster_offset;
            let chunk_size =
                (write_size - bytes_written).min(cluster_remaining.try_into().unwrap());

            // Write data to the current cluster
            write_data(
                self.current_cluster,
                self.cluster_offset,
                &buffer[bytes_written..bytes_written + chunk_size],
            )?;

            bytes_written += chunk_size;
            self.position += u64::try_from(chunk_size).unwrap();
            self.cluster_offset += u32::try_from(chunk_size).unwrap();

            // Update file size if necessary
            if self.position > self.size {
                self.size = self.position;
            }

            // Move to the next cluster if necessary
            if self.cluster_offset >= self.bytes_per_cluster && bytes_written < write_size {
                match self.fat.get(self.current_cluster)? {
                    FatEntry::Next(next) => {
                        self.current_cluster = next;
                    }
                    FatEntry::EndOfChain => {
                        // Need to allocate a new cluster
                        let new_cluster = self.fat.alloc_cluster()?;
                        self.fat
                            .set(self.current_cluster, FatEntry::Next(new_cluster))?;
                        self.current_cluster = new_cluster;
                    }
                    _ => return Err(FatError::InvalidCluster),
                }
                self.cluster_offset = 0;
            }
        }

        Ok(bytes_written)
    }

    /// Truncates the file to the specified size
    pub fn truncate(&mut self, new_size: u64) -> FatResult<()> {
        if new_size >= self.size {
            // Nothing to do if new size is larger than current size
            return Ok(());
        }

        // If truncating to 0, just free all clusters
        if new_size == 0 {
            if self.first_cluster.value() != 0 {
                self.fat.free_cluster_chain(self.first_cluster)?;
                self.first_cluster = Cluster::new(0);
                self.current_cluster = Cluster::new(0);
                self.cluster_offset = 0;
            }
            self.size = 0;
            self.position = 0;
            return Ok(());
        }

        // Seek to the new size position
        self.seek(new_size)?;

        // Free any clusters after this position
        if let FatEntry::Next(next) = self.fat.get(self.current_cluster)? {
            // Mark current cluster as end of chain
            self.fat.set(self.current_cluster, FatEntry::EndOfChain)?;

            // Free the rest of the chain
            self.fat.free_cluster_chain(next)?;
        }

        // Update the size
        self.size = new_size;
        if self.position > new_size {
            self.position = new_size;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::fat::{FatType, fat::FatEntry};

    struct MockFat {
        // Simple in-memory fat table for testing
        entries: Vec<FatEntry>,
        fat_type: FatType,
    }

    impl MockFat {
        fn new(fat_type: FatType, size: usize) -> Self {
            let mut entries = Vec::with_capacity(size);

            // Initialize with free entries
            for _ in 0..size {
                entries.push(FatEntry::Free);
            }

            // First two entries are reserved
            if entries.len() > 0 {
                entries[0] = FatEntry::Reserved;
            }
            if entries.len() > 1 {
                entries[1] = FatEntry::Reserved;
            }

            Self { entries, fat_type }
        }

        // Build a chain of clusters
        fn create_chain(&mut self, len: usize) -> Cluster {
            if len == 0 {
                return Cluster::new(0);
            }

            let start_cluster = Cluster::new(2); // First valid data cluster
            let mut prev_cluster = start_cluster;

            // Mark start as used
            self.entries[2] = FatEntry::EndOfChain;

            // For longer chains, link them
            for i in 1..len {
                let cluster_idx = i + 2; // Start from cluster 2 (first valid)
                let cluster = Cluster::new(cluster_idx as u32);

                // Link previous to this one
                self.entries[prev_cluster.value() as usize] = FatEntry::Next(cluster);

                // Mark this one as end of chain
                self.entries[cluster_idx] = FatEntry::EndOfChain;

                prev_cluster = cluster;
            }

            start_cluster
        }
    }

    impl FatEntries for MockFat {
        fn fat_type(&self) -> FatType {
            self.fat_type
        }

        fn get(&self, cluster: Cluster) -> FatResult<FatEntry> {
            if cluster.value() as usize >= self.entries.len() {
                Err(FatError::OutOfBounds)
            } else {
                Ok(self.entries[cluster.value() as usize])
            }
        }

        fn set(&mut self, cluster: Cluster, entry: FatEntry) -> FatResult<()> {
            if cluster.value() as usize >= self.entries.len() {
                Err(FatError::OutOfBounds)
            } else {
                self.entries[cluster.value() as usize] = entry;
                Ok(())
            }
        }

        fn alloc_cluster(&mut self) -> FatResult<Cluster> {
            // Find first free cluster
            for (i, entry) in self.entries.iter().enumerate().skip(2) {
                if entry == &FatEntry::Free {
                    let cluster = Cluster::new(u32::try_from(i).unwrap());
                    self.entries[i] = FatEntry::EndOfChain;
                    return Ok(cluster);
                }
            }

            Err(FatError::OutOfBounds)
        }

        fn alloc_cluster_chain(&mut self, count: usize) -> FatResult<Cluster> {
            if count == 0 {
                return Err(FatError::InvalidParameter);
            }

            let free_count = self
                .entries
                .iter()
                .skip(2) // Skip reserved clusters
                .filter(|&entry| matches!(entry, FatEntry::Free))
                .count();

            if free_count < count {
                return Err(FatError::OutOfBounds);
            }

            let first = self.alloc_cluster()?;
            let mut prev = first;

            // Allocate additional clusters
            for _ in 1..count {
                let next = self.alloc_cluster()?;
                self.set(prev, FatEntry::Next(next))?;
                prev = next;
            }

            Ok(first)
        }

        fn free_cluster(&mut self, cluster: Cluster) -> FatResult<()> {
            if !cluster.is_valid(self.fat_type) {
                return Err(FatError::InvalidCluster);
            }

            if cluster.value() as usize >= self.entries.len() {
                return Err(FatError::OutOfBounds);
            }

            self.entries[cluster.value() as usize] = FatEntry::Free;
            Ok(())
        }

        fn free_cluster_chain(&mut self, start: Cluster) -> FatResult<()> {
            let mut current = start;

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
            let count = self
                .entries
                .iter()
                .skip(2) // Skip reserved clusters
                .filter(|&entry| matches!(entry, FatEntry::Free))
                .count();

            Ok(count as u32)
        }
    }

    #[test]
    fn test_file_creation() {
        let mut mock_fat = MockFat::new(FatType::Fat16, 100);
        let first_cluster = mock_fat.create_chain(3);

        let result = FatFile::new(&mut mock_fat, first_cluster, 1024, 512);

        let file = result.expect("Failed to create file");
        assert_eq!(file.size(), 1024);
        assert_eq!(file.position(), 0);
    }

    #[test]
    fn test_file_seek() {
        let mut mock_fat = MockFat::new(FatType::Fat16, 100);
        let first_cluster = mock_fat.create_chain(3); // Create a 3-cluster chain

        let mut file =
            FatFile::new(&mut mock_fat, first_cluster, 1500, 512).expect("Failed to create file");

        // Test seeking within file
        let pos = file.seek(600).expect("Failed to seek");
        assert_eq!(pos, 600);
        assert_eq!(file.position(), 600);

        // Test seeking past first cluster
        let pos = file.seek(700).expect("Failed to seek");
        assert_eq!(pos, 700);
        assert_eq!(file.position(), 700);

        // Test seeking backward
        let pos = file.seek(100).expect("Failed to seek backward");
        assert_eq!(pos, 100);
        assert_eq!(file.position(), 100);

        // Test seeking to end of file
        let pos = file.seek(1500).expect("Failed to seek to EOF");
        assert_eq!(pos, 1500);
        assert_eq!(file.position(), 1500);

        // Test seeking beyond end of file (should clamp to file size)
        let pos = file.seek(2000).expect("Failed to seek beyond EOF");
        assert_eq!(pos, 1500);
        assert_eq!(file.position(), 1500);
    }

    #[test]
    fn test_file_read_write() {
        let mut mock_fat = MockFat::new(FatType::Fat16, 100);
        let first_cluster = mock_fat.create_chain(3); // Create a 3-cluster chain

        let mut file = FatFile::new(
            &mut mock_fat,
            first_cluster,
            0,   // Empty file
            512, // 512 bytes per cluster
        )
        .expect("Failed to create file");

        // Create a buffer for writing
        let data = b"Hello, FAT filesystem!";

        // Mock data read/write callbacks
        let mut mock_data = vec![0u8; 3 * 512]; // 3 clusters

        // Write data
        let bytes_written = file
            .write(data, &mut |cluster, offset, data_slice| {
                let start = (cluster.value() - 2) as usize * 512 + offset as usize;
                let end = start + data_slice.len();
                mock_data[start..end].copy_from_slice(data_slice);
                Ok(())
            })
            .expect("Failed to write data");

        assert_eq!(bytes_written, data.len());
        assert_eq!(file.size(), data.len() as u64);

        // Seek back to beginning
        file.seek(0).expect("Failed to seek to beginning");

        // Read the data back
        let mut read_buffer = [0u8; 32];
        let bytes_read = file
            .read(&mut read_buffer, &mut |cluster, offset, data_slice| {
                let start = (cluster.value() - 2) as usize * 512 + offset as usize;
                let end = start + data_slice.len();
                data_slice.copy_from_slice(&mock_data[start..end]);
                Ok(())
            })
            .expect("Failed to read data");

        assert_eq!(bytes_read, data.len());
        assert_eq!(&read_buffer[..bytes_read], data);
    }

    #[test]
    fn test_file_truncate() {
        let mut mock_fat = MockFat::new(FatType::Fat16, 100);
        let first_cluster = mock_fat.create_chain(3); // Create a 3-cluster chain

        let mut file =
            FatFile::new(&mut mock_fat, first_cluster, 1500, 512).expect("Failed to create file");

        // Truncate to smaller size
        file.truncate(700).expect("Failed to truncate");
        assert_eq!(file.size(), 700);

        // Verify position is clamped if needed
        file.seek(1000).expect("Failed to seek");
        assert_eq!(file.position(), 700);

        // Truncate to 0
        file.truncate(0).expect("Failed to truncate to zero");
        assert_eq!(file.size(), 0);
        assert_eq!(file.position(), 0);
    }

    #[test]
    fn test_file_cluster_allocation() {
        let mut mock_fat = MockFat::new(FatType::Fat16, 100);
        let mut file = FatFile::new(
            &mut mock_fat,
            Cluster::new(0), // No initial cluster
            0,
            512,
        )
        .expect("Failed to create file");

        // Mock data callback
        let mut mock_data = vec![0u8; 3 * 512]; // Space for 3 clusters
        let write_fn = &mut |cluster: Cluster, offset: u32, data_slice: &[u8]| {
            let start = (cluster.value() - 2) as usize * 512 + offset as usize;
            let end = start + data_slice.len();
            mock_data[start..end].copy_from_slice(data_slice);
            Ok(())
        };

        // Write data that spans multiple clusters
        let data = vec![1u8; 1000]; // 1000 bytes of data

        // First write should allocate a cluster
        let bytes_written = file.write(&data, write_fn).expect("Failed to write data");

        assert_eq!(bytes_written, data.len());
        assert_eq!(file.size(), data.len() as u64);

        // Verify that a cluster was allocated
        assert_ne!(file.first_cluster, Cluster::new(0));
    }
}
