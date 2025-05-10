use super::{
    BoxedDataReader, Cluster, FatError, FatResult, FatType, RefDataReader, RefDataWriter,
    date::{Date, DateTime, Time},
    dirent::{Attributes, DirEntry, LongNameEntry, calc_short_name_checksum},
    fat::{FatEntries, FatEntry},
};
use alloc::{
    boxed::Box,
    string::{String, ToString as _},
    vec::Vec,
};

/// A directory in a FAT filesystem
pub struct Directory<'a, T: FatEntries> {
    /// The FAT entries
    fat: &'a mut T,
    /// First cluster of the directory
    first_cluster: Cluster,
    /// Current position (entry index)
    position: usize,
    /// Bytes per cluster
    bytes_per_cluster: u32,
    /// Current cluster being read
    current_cluster: Cluster,
    /// Offset within current cluster in bytes
    cluster_offset: u32,
}

/// Directory entry iterator
pub struct DirEntryIterator<'a, 'b, T: FatEntries> {
    dir: &'a mut Directory<'b, T>,
    read_data: BoxedDataReader<'a>,
}

/// Directory entry with long filename support
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    /// The short name entry
    short_entry: DirEntry,
    /// Long filename (if available)
    long_name: Option<String>,
}

impl<'a, T: FatEntries> Directory<'a, T> {
    /// Creates a new directory handle
    pub fn new(fat: &'a mut T, first_cluster: Cluster, bytes_per_cluster: u32) -> FatResult<Self> {
        if !first_cluster.is_valid(fat.fat_type()) && first_cluster.value() != 0 {
            return Err(FatError::InvalidCluster);
        }

        Ok(Self {
            fat,
            first_cluster,
            position: 0,
            bytes_per_cluster,
            current_cluster: first_cluster,
            cluster_offset: 0,
        })
    }

    #[must_use]
    #[inline]
    /// Returns the current position (entry index)
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Seeks to a specific entry by index
    pub fn seek(&mut self, position: usize) -> FatResult<usize> {
        // If moving backwards, start from beginning
        if position < self.position {
            self.current_cluster = self.first_cluster;
            self.cluster_offset = 0;
            self.position = 0;
        }

        // Calculate how many entries to skip
        let entries_to_skip = position - self.position;
        if entries_to_skip == 0 {
            return Ok(self.position);
        }

        // Skip entries by advancing the position
        let entries_per_cluster =
            usize::try_from(self.bytes_per_cluster).unwrap() / size_of::<DirEntry>();
        let mut remaining = entries_to_skip;

        while remaining > 0 {
            // Calculate remaining entries in the current cluster
            let entries_in_current =
                entries_per_cluster - (self.cluster_offset as usize / size_of::<DirEntry>());

            if remaining <= entries_in_current {
                // We can fit within the current cluster
                self.cluster_offset += u32::try_from(remaining * size_of::<DirEntry>()).unwrap();
                self.position += remaining;
                remaining = 0;
            } else {
                // We need to move to the next cluster
                remaining -= entries_in_current;
                self.position += entries_in_current;

                match self.fat.get(self.current_cluster)? {
                    FatEntry::Next(next) => {
                        self.current_cluster = next;
                        self.cluster_offset = 0;
                    }
                    FatEntry::EndOfChain => {
                        // We can't move beyond the end of the directory
                        return Ok(self.position);
                    }
                    _ => return Err(FatError::InvalidCluster),
                }
            }
        }

        Ok(self.position)
    }

    #[inline]
    /// Resets the position to the beginning of the directory
    pub const fn rewind(&mut self) -> FatResult<()> {
        self.current_cluster = self.first_cluster;
        self.cluster_offset = 0;
        self.position = 0;
        Ok(())
    }

    #[inline]
    /// Returns an iterator over directory entries
    pub fn entries<'b>(
        &'b mut self,
        read_data: impl FnMut(Cluster, u32, &mut [u8]) -> FatResult<()> + 'b,
    ) -> DirEntryIterator<'b, 'a, T> {
        DirEntryIterator {
            dir: self,
            read_data: Box::new(read_data),
        }
    }

    /// Reads a single directory entry
    pub fn read_entry(&mut self, read_data: RefDataReader) -> FatResult<Option<DirectoryEntry>> {
        let mut entry_data = [0u8; size_of::<DirEntry>()];

        // Read the directory entry data
        self.read_entry_data(&mut entry_data, read_data)?;

        // Check if we reached the end of the directory
        if entry_data[0] == DirEntry::END_OF_ENTRIES {
            return Ok(None);
        }

        // Skip deleted entries
        if entry_data[0] == DirEntry::DELETED_ENTRY {
            self.position += 1;
            return self.read_entry(read_data);
        }

        // Parse the directory entry
        let entry = unsafe { entry_data.as_ptr().cast::<DirEntry>().read() };

        // Check if this is a long filename entry
        if entry.is_long_name() {
            // Handle long filename entry
            let long_entry = unsafe { entry_data.as_ptr().cast::<LongNameEntry>().read() };

            // Read all LFN entries until we get to the short name entry
            let mut lfn_entries = Vec::new();
            lfn_entries.push(long_entry);

            while lfn_entries.last().unwrap().is_last() {
                // Read next entry
                let mut next_entry_data = [0u8; size_of::<DirEntry>()];
                self.read_entry_data(&mut next_entry_data, read_data)?;

                let next_entry = unsafe { entry_data.as_ptr().cast::<DirEntry>().read() };

                if next_entry.is_long_name() {
                    let next_lfn = unsafe { entry_data.as_ptr().cast::<LongNameEntry>().read() };
                    lfn_entries.push(next_lfn);
                } else {
                    // Extract the long filename
                    let long_name = build_long_filename(&lfn_entries)?;

                    // Return the combined entry
                    return Ok(Some(DirectoryEntry {
                        short_entry: next_entry,
                        long_name: Some(long_name),
                    }));
                }
            }

            // If we get here, there was an error in the LFN chain
            return Err(FatError::InvalidDirEntry);
        }

        self.position += 1;

        Ok(Some(DirectoryEntry {
            short_entry: entry,
            long_name: None,
        }))
    }

    /// Reads raw entry data
    fn read_entry_data(&mut self, buffer: &mut [u8], read_data: RefDataReader) -> FatResult<()> {
        // Read the entry data from the current position
        read_data(self.current_cluster, self.cluster_offset, buffer)?;

        // Advance the position
        self.cluster_offset += u32::try_from(buffer.len()).unwrap();

        // Check if we need to move to the next cluster
        if self.cluster_offset >= self.bytes_per_cluster {
            match self.fat.get(self.current_cluster)? {
                FatEntry::Next(next) => {
                    self.current_cluster = next;
                    self.cluster_offset = 0;
                }
                FatEntry::EndOfChain => {
                    // End of directory reached
                }
                _ => return Err(FatError::InvalidCluster),
            }
        }

        Ok(())
    }

    /// Writes a new directory entry
    pub fn write_entry(
        &mut self,
        entry: &DirectoryEntry,
        write_data: RefDataWriter,
    ) -> FatResult<()> {
        // If there's a long filename, write LFN entries first
        if let Some(ref long_name) = entry.long_name {
            // Calculate number of LFN entries needed
            let chars = long_name.encode_utf16().collect::<Vec<u16>>();
            let lfn_entries_count =
                u8::try_from(chars.len().div_ceil(LongNameEntry::CHARS_PER_ENTRY)).unwrap();

            // Calculate checksum for the short name
            let checksum = calc_short_name_checksum(&entry.short_entry.filename_raw());

            // Write LFN entries in reverse order
            for i in 0..lfn_entries_count {
                let seq_num = lfn_entries_count - i;
                let is_last = i == 0;

                let mut lfn_entry = LongNameEntry::new(seq_num, checksum, is_last);

                // Fill in the name parts
                let start_idx = usize::from(i) * LongNameEntry::CHARS_PER_ENTRY;
                for j in 0..LongNameEntry::CHARS_PER_ENTRY {
                    let char_idx = start_idx + j;
                    if char_idx < chars.len() {
                        lfn_entry.set_name(j, chars[char_idx])?;
                    } else if j < LongNameEntry::CHARS_PER_ENTRY - 1 {
                        // Fill with 0x0000 (end of name)
                        lfn_entry.set_name(j, 0)?;
                    } else {
                        // Fill the last character with 0xFFFF (required)
                        lfn_entry.set_name(j, 0xFFFF)?;
                    }
                }

                // Write the LFN entry
                let lfn_bytes = unsafe {
                    core::slice::from_raw_parts(
                        (&raw const lfn_entry).cast::<u8>(),
                        size_of::<LongNameEntry>(),
                    )
                };

                let mut lfn_buffer = lfn_bytes.to_vec();
                write_data(self.current_cluster, self.cluster_offset, &mut lfn_buffer)?;

                // Advance position
                self.cluster_offset += u32::try_from(lfn_bytes.len()).unwrap();

                // Check if we need to move to the next cluster
                if self.cluster_offset >= self.bytes_per_cluster {
                    match self.fat.get(self.current_cluster)? {
                        FatEntry::Next(next) => {
                            self.current_cluster = next;
                            self.cluster_offset = 0;
                        }
                        FatEntry::EndOfChain => {
                            // Allocate a new cluster
                            let new_cluster = self.fat.alloc_cluster()?;
                            self.fat
                                .set(self.current_cluster, FatEntry::Next(new_cluster))?;
                            self.current_cluster = new_cluster;
                            self.cluster_offset = 0;
                        }
                        _ => return Err(FatError::InvalidCluster),
                    }
                }
            }
        }

        // Write the short name entry
        let entry_bytes = unsafe {
            core::slice::from_raw_parts(
                (&raw const entry.short_entry).cast::<u8>(),
                size_of::<DirEntry>(),
            )
        };

        let mut entry_buffer = entry_bytes.to_vec();
        write_data(self.current_cluster, self.cluster_offset, &mut entry_buffer)?;

        // Advance position
        self.cluster_offset += u32::try_from(entry_bytes.len()).unwrap();

        // Check if we need to move to the next cluster
        if self.cluster_offset >= self.bytes_per_cluster {
            match self.fat.get(self.current_cluster)? {
                FatEntry::Next(next) => {
                    self.current_cluster = next;
                    self.cluster_offset = 0;
                }
                FatEntry::EndOfChain => {
                    // End of directory reached
                }
                _ => return Err(FatError::InvalidCluster),
            }
        }

        Ok(())
    }
}

// Helper functions
fn build_long_filename(entries: &[LongNameEntry]) -> FatResult<String> {
    // Sort entries by sequence number (ascending)
    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by_key(super::dirent::LongNameEntry::seq_num);

    let mut result = String::with_capacity(sorted_entries.len() * LongNameEntry::CHARS_PER_ENTRY);

    // Process each entry
    for entry in &sorted_entries {
        // Extract characters from this entry
        for i in 0..LongNameEntry::CHARS_PER_ENTRY {
            match entry.get_name(i)? {
                0 => break,  // End of name
                0xFFFF => {} // Skip padding
                ch => {
                    if let Some(c) = char::from_u32(u32::from(ch)) {
                        result.push(c);
                    }
                }
            }
        }
    }

    Ok(result)
}

impl<T: FatEntries> Iterator for DirEntryIterator<'_, '_, T> {
    type Item = FatResult<DirectoryEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.dir.read_entry(&mut self.read_data) {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => None, // End of directory
            Err(e) => Some(Err(e)),
        }
    }
}

impl DirectoryEntry {
    /// Creates a new directory entry
    pub fn new(name: &str, attributes: Attributes) -> FatResult<Self> {
        // Create a short entry
        let mut short_entry = DirEntry::new();
        short_entry.set_attributes(attributes);

        // Generate a short filename (8.3 format)
        // For simplicity, this implementation just uses the first 8 chars for name
        // and first 3 for extension
        let name_parts: Vec<&str> = name.split('.').collect();
        let base_name = name_parts[0];
        let extension = if name_parts.len() > 1 {
            name_parts[1]
        } else {
            ""
        };

        // Fill the short name fields
        let mut name_buf = [b' '; 8];
        let mut ext_buf = [b' '; 3];

        for (i, &b) in base_name.as_bytes().iter().take(8).enumerate() {
            name_buf[i] = b.to_ascii_uppercase();
        }

        for (i, &b) in extension.as_bytes().iter().take(3).enumerate() {
            ext_buf[i] = b.to_ascii_uppercase();
        }

        // Create the new entry
        let mut entry = Self {
            short_entry,
            long_name: Some(name.to_string()),
        };

        // Initialize with current date/time
        let today = Date::new(2025, 1, 1); // TODO: Replace with actual current date
        let now = Time::new(0, 0, 0, 0); // TODO: Replace with actual current time

        entry
            .short_entry
            .set_creation_datetime(DateTime::new(today, now));
        entry.short_entry.set_last_access_date(today);
        entry.short_entry.set_last_write_datetime(today, now);

        Ok(entry)
    }

    #[must_use]
    /// Gets the name of the entry (long name if available, otherwise short name)
    pub fn name(&self) -> String {
        self.long_name.as_ref().map_or_else(
            || {
                // Convert the short name to a string
                let name = self.short_entry.name();
                let ext = self.short_entry.extension();
                // Trim spaces from name and extension
                let name_end = name.iter().position(|&b| b == b' ').unwrap_or(name.len());
                let ext_end = ext.iter().position(|&b| b == b' ').unwrap_or(ext.len());
                let name_str = core::str::from_utf8(&name[..name_end]).unwrap_or("Invalid");
                if ext_end > 0 {
                    let ext_str = core::str::from_utf8(&ext[..ext_end]).unwrap_or("Invalid");
                    alloc::format!("{name_str}.{ext_str}")
                } else {
                    name_str.to_string()
                }
            },
            Clone::clone,
        )
    }

    #[must_use]
    #[inline]
    /// Returns whether the entry is a directory
    pub const fn is_directory(&self) -> bool {
        self.short_entry.is_directory()
    }

    #[must_use]
    #[inline]
    /// Returns whether the entry is a file
    pub const fn is_file(&self) -> bool {
        self.short_entry.is_file()
    }

    #[must_use]
    #[inline]
    /// Returns the attributes
    pub const fn attributes(&self) -> Attributes {
        self.short_entry.attributes()
    }

    #[must_use]
    #[inline]
    /// Returns the first cluster
    pub fn first_cluster(&self, fat_type: FatType) -> Cluster {
        self.short_entry.first_cluster(fat_type)
    }

    #[must_use]
    #[inline]
    /// Returns the file size
    pub const fn file_size(&self) -> u32 {
        self.short_entry.file_size()
    }

    #[inline]
    /// Sets the first cluster
    pub fn set_first_cluster(&mut self, cluster: Cluster, fat_type: FatType) {
        self.short_entry.set_first_cluster(cluster, fat_type);
    }

    #[inline]
    /// Sets the file size
    pub const fn set_file_size(&mut self, size: u32) {
        self.short_entry.set_file_size(size);
    }
}
