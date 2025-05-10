use super::{
    Cluster, FatError, FatResult, FatType,
    date::{Date, DateTime, DosDate, DosDateTime, DosTime, Time},
};

/// Size of a directory entry in bytes (always 32 bytes)
pub const DIR_ENTRY_SIZE: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Directory entry attributes
pub struct Attributes(u8);

impl Attributes {
    /// Read-only attribute
    pub const READ_ONLY: u8 = 0x01;
    /// Hidden attribute
    pub const HIDDEN: u8 = 0x02;
    /// System attribute
    pub const SYSTEM: u8 = 0x04;
    /// Volume ID attribute
    pub const VOLUME_ID: u8 = 0x08;
    /// Directory attribute
    pub const DIRECTORY: u8 = 0x10;
    /// Archive attribute
    pub const ARCHIVE: u8 = 0x20;
    /// Long file name attribute
    pub const LONG_NAME: u8 = Self::READ_ONLY | Self::HIDDEN | Self::SYSTEM | Self::VOLUME_ID;
    /// Long file name mask
    pub const LONG_NAME_MASK: u8 = Self::READ_ONLY
        | Self::HIDDEN
        | Self::SYSTEM
        | Self::VOLUME_ID
        | Self::DIRECTORY
        | Self::ARCHIVE;

    #[must_use]
    #[inline]
    /// Creates a new attribute set
    pub const fn new(attributes: u8) -> Self {
        Self(attributes)
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is read-only
    pub const fn is_read_only(&self) -> bool {
        self.0 & Self::READ_ONLY != 0
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is hidden
    pub const fn is_hidden(&self) -> bool {
        self.0 & Self::HIDDEN != 0
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is a system file
    pub const fn is_system(&self) -> bool {
        self.0 & Self::SYSTEM != 0
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is a volume ID
    pub const fn is_volume_id(&self) -> bool {
        self.0 & Self::VOLUME_ID != 0
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is a directory
    pub const fn is_directory(&self) -> bool {
        self.0 & Self::DIRECTORY != 0
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is archived
    pub const fn is_archive(&self) -> bool {
        self.0 & Self::ARCHIVE != 0
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is a long file name
    pub const fn is_long_name(&self) -> bool {
        (self.0 & Self::LONG_NAME_MASK) == Self::LONG_NAME
    }
}

/// FAT directory entry
#[derive(Default, Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct DirEntry {
    /// Filename (8 bytes)
    name: [u8; 8],
    /// Extension (3 bytes)
    ext: [u8; 3],
    /// File attributes
    attr: u8,
    /// Reserved for Windows NT
    nt_res: u8,
    /// Creation time
    creation_time: DosTime,
    /// Creation date
    creation_date: DosDate,
    /// Last access date
    last_access_date: DosDate,
    /// High word of first cluster number for FAT32
    first_cluster_high: u16,
    /// Last modification time
    write_time: u16,
    /// Last modification date
    write_date: DosDate,
    /// Low word of first cluster number
    first_cluster_low: u16,
    /// File size in bytes
    file_size: u32,
}

impl DirEntry {
    /// Deleted entry marker (first byte)
    pub const DELETED_ENTRY: u8 = 0xE5;
    /// End of directory marker (first byte)
    pub const END_OF_ENTRIES: u8 = 0x00;
    /// Dot entry (current directory)
    pub const DOT_ENTRY: &'static [u8; 11] = b".          ";
    /// Dotdot entry (parent directory)
    pub const DOTDOT_ENTRY: &'static [u8; 11] = b"..         ";

    #[must_use]
    #[inline]
    /// Creates a new directory entry
    pub const fn new() -> Self {
        Self {
            name: [0; 8],
            ext: [0; 3],
            attr: 0,
            nt_res: 0,
            creation_time: DosTime::BASE,
            creation_date: DosDate::BASE,
            last_access_date: DosDate::BASE,
            first_cluster_high: 0,
            write_time: 0,
            write_date: DosDate::BASE,
            first_cluster_low: 0,
            file_size: 0,
        }
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is free (unused)
    pub const fn is_free(&self) -> bool {
        self.name[0] == Self::END_OF_ENTRIES
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is deleted
    pub const fn is_deleted(&self) -> bool {
        self.name[0] == Self::DELETED_ENTRY
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is valid
    pub const fn is_valid(&self) -> bool {
        !self.is_free() && !self.is_deleted()
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is a directory
    pub const fn is_directory(&self) -> bool {
        Attributes::new(self.attr).is_directory()
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is a file
    pub const fn is_file(&self) -> bool {
        self.is_valid() && !self.is_directory() && !self.is_volume_id()
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is a volume ID
    pub const fn is_volume_id(&self) -> bool {
        Attributes::new(self.attr).is_volume_id()
    }

    #[must_use]
    #[inline]
    /// Returns true if the entry is a long filename
    pub const fn is_long_name(&self) -> bool {
        Attributes::new(self.attr).is_long_name()
    }

    #[must_use]
    #[inline]
    /// Returns the file attributes
    pub const fn attributes(&self) -> Attributes {
        Attributes::new(self.attr)
    }

    #[inline]
    /// Sets the file attributes
    pub const fn set_attributes(&mut self, attributes: Attributes) {
        self.attr = attributes.0;
    }

    #[must_use]
    #[inline]
    /// Returns the name of the file (without extension)
    pub const fn name(&self) -> [u8; 8] {
        self.name
    }

    #[must_use]
    #[inline]
    /// Returns the extension of the file
    pub const fn extension(&self) -> [u8; 3] {
        self.ext
    }

    #[must_use]
    /// Returns the full 8.3 filename as a slice
    pub fn filename_raw(&self) -> [u8; 11] {
        let mut result = [0u8; 11];
        result[..8].copy_from_slice(&self.name);
        result[8..].copy_from_slice(&self.ext);
        result
    }

    #[must_use]
    /// Returns the first cluster number
    pub fn first_cluster(&self, fat_type: FatType) -> Cluster {
        let low = u32::from(self.first_cluster_low);
        let high = match fat_type {
            FatType::Fat32 => u32::from(self.first_cluster_high) << 16,
            _ => 0,
        };
        Cluster::new(low | high)
    }

    /// Sets the first cluster number
    pub fn set_first_cluster(&mut self, cluster: Cluster, fat_type: FatType) {
        self.first_cluster_low = (cluster.value() & 0xFFFF) as u16;
        if fat_type == FatType::Fat32 {
            self.first_cluster_high = ((cluster.value() >> 16) & 0xFFFF) as u16;
        }
    }

    #[must_use]
    #[inline]
    /// Returns the file size
    pub const fn file_size(&self) -> u32 {
        self.file_size
    }

    #[inline]
    /// Sets the file size
    pub const fn set_file_size(&mut self, size: u32) {
        self.file_size = size;
    }

    #[must_use]
    #[inline]
    /// Returns the creation date and time
    pub fn creation_datetime(&self) -> DateTime {
        DateTime::decode(DosDateTime::new(self.creation_date, self.creation_time))
    }

    #[inline]
    /// Sets the creation date and time
    pub fn set_creation_datetime(&mut self, datetime: DateTime) {
        let dos_datetime = datetime.encode();
        self.creation_date = dos_datetime.dos_date();
        self.creation_time = dos_datetime.dos_time();
    }

    #[must_use]
    #[inline]
    /// Returns the last access date
    pub fn last_access_date(&self) -> Date {
        Date::decode(self.last_access_date)
    }

    #[inline]
    /// Sets the last access date
    pub fn set_last_access_date(&mut self, date: Date) {
        self.last_access_date = date.encode();
    }

    #[must_use]
    #[inline]
    /// Returns the last write date and time
    pub fn last_write_datetime(&self) -> DateTime {
        DateTime::decode(DosDateTime::new(
            self.write_date,
            DosTime::new(self.write_time, 0),
        ))
    }

    #[inline]
    /// Sets the last write date and time
    pub fn set_last_write_datetime(&mut self, date: Date, time: Time) {
        self.write_date = date.encode();
        self.write_time = time.encode().dos_time();
    }
}

/// Entry for long file name
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct LongNameEntry {
    /// Sequence number (0-based, bit 6 set for last entry)
    seq_num: u8,
    /// First 5 characters of long name (UTF-16)
    name1: [u8; 10],
    /// Attributes (always 0x0F for LFN)
    attr: u8,
    /// Entry type (always 0 for LFN)
    entry_type: u8,
    /// Checksum of short name
    checksum: u8,
    /// Next 6 characters of long name
    name2: [u8; 12],
    /// First cluster (always 0 for LFN)
    first_cluster: u16,
    /// Last 2 characters of long name
    name3: [u8; 4],
}

impl LongNameEntry {
    /// Last entry marker in sequence number
    pub const LAST_ENTRY: u8 = 0x40;
    /// Character count per LFN entry
    pub const CHARS_PER_ENTRY: usize = 13;

    #[must_use]
    #[inline]
    /// Creates a new long name entry
    pub const fn new(seq_num: u8, checksum: u8, is_last: bool) -> Self {
        let mut seq = seq_num;
        if is_last {
            seq |= Self::LAST_ENTRY;
        }

        Self {
            seq_num: seq,
            name1: [0; 10],
            attr: Attributes::LONG_NAME,
            entry_type: 0,
            checksum,
            name2: [0; 12],
            first_cluster: 0,
            name3: [0; 4],
        }
    }

    #[must_use]
    #[inline]
    /// Returns true if this is the last entry in the long name sequence
    pub const fn is_last(&self) -> bool {
        self.seq_num & Self::LAST_ENTRY != 0
    }

    #[must_use]
    #[inline]
    /// Returns the sequence number
    pub const fn seq_num(&self) -> u8 {
        self.seq_num & !Self::LAST_ENTRY
    }

    #[must_use]
    #[inline]
    /// Returns the checksum
    pub const fn checksum(&self) -> u8 {
        self.checksum
    }

    /// Sets the name part at the given index (0-12)
    pub const fn set_name(&mut self, idx: usize, ch: u16) -> FatResult<()> {
        if idx >= Self::CHARS_PER_ENTRY {
            return Err(FatError::InvalidParameter);
        }

        let bytes = ch.to_le_bytes();

        if idx < 5 {
            self.name1[idx * 2] = bytes[0];
            self.name1[idx * 2 + 1] = bytes[1];
        } else if idx < 11 {
            let idx = idx - 5;
            self.name2[idx * 2] = bytes[0];
            self.name2[idx * 2 + 1] = bytes[1];
        } else {
            let idx = idx - 11;
            self.name3[idx * 2] = bytes[0];
            self.name3[idx * 2 + 1] = bytes[1];
        }

        Ok(())
    }

    /// Gets the name part at the given index (0-12)
    pub const fn get_name(&self, idx: usize) -> FatResult<u16> {
        if idx >= Self::CHARS_PER_ENTRY {
            return Err(FatError::InvalidParameter);
        }

        let bytes = if idx < 5 {
            [self.name1[idx * 2], self.name1[idx * 2 + 1]]
        } else if idx < 11 {
            let idx = idx - 5;
            [self.name2[idx * 2], self.name2[idx * 2 + 1]]
        } else {
            let idx = idx - 11;
            [self.name3[idx * 2], self.name3[idx * 2 + 1]]
        };

        Ok(u16::from_le_bytes(bytes))
    }
}

/// Calculate the checksum for a 8.3 filename
pub(crate) fn calc_short_name_checksum(name: &[u8; 11]) -> u8 {
    let mut sum: u8 = 0;
    for &b in name {
        sum = ((sum & 1) << 7).wrapping_add(sum >> 1).wrapping_add(b);
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::fat::date::{Date, DateTime, Time};

    #[test]
    fn test_attributes() {
        let attr = Attributes::new(Attributes::READ_ONLY | Attributes::HIDDEN);
        assert!(attr.is_read_only());
        assert!(attr.is_hidden());
        assert!(!attr.is_system());
        assert!(!attr.is_volume_id());
        assert!(!attr.is_directory());
        assert!(!attr.is_archive());
        assert!(!attr.is_long_name());

        let long_name = Attributes::new(Attributes::LONG_NAME);
        assert!(long_name.is_long_name());
        assert!(!long_name.is_directory());

        let dir = Attributes::new(Attributes::DIRECTORY);
        assert!(dir.is_directory());
        assert!(!dir.is_long_name());
    }

    #[test]
    fn test_dir_entry() {
        let mut entry = DirEntry::new();

        // Test defaults
        assert!(entry.is_free());
        assert!(!entry.is_deleted());
        assert!(!entry.is_valid());

        // Test filename manipulation
        let name = *b"TEST    ";
        let ext = *b"TXT";
        entry.name = name;
        entry.ext = ext;
        assert_eq!(entry.name(), name);
        assert_eq!(entry.extension(), ext);

        let filename_raw = entry.filename_raw();
        assert_eq!(&filename_raw[0..8], &name);
        assert_eq!(&filename_raw[8..11], &ext);

        // Test cluster handling
        let cluster = Cluster::new(0x1234);
        entry.set_first_cluster(cluster, FatType::Fat16);
        assert_eq!(entry.first_cluster(FatType::Fat16), cluster);

        let cluster32 = Cluster::new(0x12345678);
        entry.set_first_cluster(cluster32, FatType::Fat32);
        assert_eq!(entry.first_cluster(FatType::Fat32), cluster32);

        // Test file size
        entry.set_file_size(12345);
        assert_eq!(entry.file_size(), 12345);

        // Test date/time
        let date = Date::new(2023, 3, 15);
        let time = Time::new(14, 30, 45, 0);
        let datetime = DateTime::new(date, time);
        entry.set_creation_datetime(datetime);
        assert_eq!(entry.creation_datetime(), datetime);

        entry.set_last_access_date(date);
        assert_eq!(entry.last_access_date(), date);

        entry.set_last_write_datetime(date, time);
        assert_eq!(entry.last_write_datetime().date(), date);
        assert_eq!(entry.last_write_datetime().time().hour(), time.hour());
        assert_eq!(entry.last_write_datetime().time().min(), time.min());
    }

    #[test]
    fn test_long_name_entry() {
        let mut lfn = LongNameEntry::new(1, 0x12, true);

        // Test sequence number and checksum
        assert_eq!(lfn.seq_num(), 1);
        assert!(lfn.is_last());
        assert_eq!(lfn.checksum(), 0x12);

        // Test name storage and retrieval
        assert!(lfn.set_name(0, 'T' as u16).is_ok());
        assert!(lfn.set_name(1, 'e' as u16).is_ok());
        assert!(lfn.set_name(2, 's' as u16).is_ok());
        assert!(lfn.set_name(3, 't' as u16).is_ok());

        assert_eq!(lfn.get_name(0).unwrap(), 'T' as u16);
        assert_eq!(lfn.get_name(1).unwrap(), 'e' as u16);
        assert_eq!(lfn.get_name(2).unwrap(), 's' as u16);
        assert_eq!(lfn.get_name(3).unwrap(), 't' as u16);

        // Test bounds checking
        assert!(
            lfn.set_name(LongNameEntry::CHARS_PER_ENTRY, 'X' as u16)
                .is_err()
        );
        assert!(lfn.get_name(LongNameEntry::CHARS_PER_ENTRY).is_err());
    }

    #[test]
    fn test_short_name_checksum() {
        let name = *b"TEST    TXT";
        assert_eq!(calc_short_name_checksum(&name), 143);

        let name2 = *b"README  TXT";
        assert_eq!(calc_short_name_checksum(&name2), 115);
    }
}
