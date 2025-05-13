use beskar_core::static_assert;

/// BIOS Parameter Block (BPB) start.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct BootParamBlockStart {
    /// Bytes per sector.
    ///
    /// This field is either 512, 1024, 2048, or 4096.
    bytes_per_sector: u16,
    /// Sectors per cluster.
    sectors_per_cluster: u8,
    /// Reserved sectors.
    reserved_sectors: u16,
    /// Number of FATs.
    fat_count: u8,
    root_entries: u16,
    /// Total sectors in the file system.
    ///
    /// If the total number of sectors exceeds `u16::MAX`, this field is set to 0
    /// and one should use `total_sectors_large` instead.
    total_sectors: u16,
    /// Driver type.
    ///
    /// Example: 0xF8 for fixed disk and 0xF0 for removable disk.
    media_descriptor: u8,
    /// Sectors per FAT.
    ///
    /// DO NOT USE THIS FIELD FOR FAT32 FILE SYSTEMS.
    sectors_per_fat: u16,
    /// Sectors per track.
    sectors_per_track: u16,
    /// Number of heads.
    heads: u16,
    /// Hidden sectors.
    hidden_sectors: u32,
    /// Total sectors in the file system.
    ///
    /// This field is used when `total_sectors` is set to 0.
    total_sectors_large: u32,
}

impl Default for BootParamBlockStart {
    fn default() -> Self {
        Self::new()
    }
}

impl BootParamBlockStart {
    #[must_use]
    #[inline]
    /// Create a new `BootParamBlockStart` with default values
    pub const fn new() -> Self {
        Self {
            bytes_per_sector: 512,  // Standard sector size
            sectors_per_cluster: 1, // Default for small partitions
            reserved_sectors: 1,    // Minimum required is 1 for the boot sector
            fat_count: 2,           // Standard is 2 FATs for redundancy
            root_entries: 512,      // Default for FAT12/16
            total_sectors: 0,       // Will be set based on capacity
            media_descriptor: 0xF8, // Fixed disk
            sectors_per_fat: 0,     // Will be calculated based on volume size
            sectors_per_track: 63,  // Common value for modern disks
            heads: 255,             // Common value for modern disks
            hidden_sectors: 0,      // Usually 0 for primary partitions
            total_sectors_large: 0, // Will be set based on capacity
        }
    }

    #[must_use]
    #[inline]
    /// Set the bytes per sector
    pub const fn with_bytes_per_sector(mut self, bytes_per_sector: u16) -> Self {
        // Only allow valid values: 512, 1024, 2048, 4096
        assert!(
            bytes_per_sector.is_power_of_two()
                && 512 <= bytes_per_sector
                && bytes_per_sector <= 4096
        );

        self.bytes_per_sector = bytes_per_sector;
        self
    }

    #[must_use]
    #[inline]
    /// Set the sectors per cluster
    pub const fn with_sectors_per_cluster(mut self, sectors_per_cluster: u8) -> Self {
        assert!(sectors_per_cluster.is_power_of_two());

        self.sectors_per_cluster = sectors_per_cluster;
        self
    }

    #[must_use]
    #[inline]
    /// Set the reserved sectors
    pub const fn with_reserved_sectors(mut self, reserved_sectors: u16) -> Self {
        assert!(reserved_sectors > 0);

        self.reserved_sectors = reserved_sectors;
        self
    }

    #[must_use]
    #[inline]
    /// Set the number of FATs
    pub const fn with_fat_count(mut self, fat_count: u8) -> Self {
        assert!(fat_count > 0);

        self.fat_count = fat_count;
        self
    }

    #[must_use]
    #[inline]
    /// Set the root entries
    pub const fn with_root_entries(mut self, root_entries: u16) -> Self {
        self.root_entries = root_entries;
        self
    }

    #[must_use]
    #[inline]
    /// Set the total sectors (small count, use `with_total_sectors_large` for large volumes)
    pub const fn with_total_sectors(mut self, total_sectors: u16) -> Self {
        self.total_sectors = total_sectors;
        if total_sectors != 0 {
            self.total_sectors_large = 0; // Only one of these should be non-zero
        }
        self
    }

    #[must_use]
    #[inline]
    /// Set the media descriptor
    pub const fn with_media_descriptor(mut self, media_descriptor: u8) -> Self {
        self.media_descriptor = media_descriptor;
        self
    }

    #[must_use]
    #[inline]
    /// Set the sectors per FAT (for FAT12/16 only)
    pub const fn with_sectors_per_fat(mut self, sectors_per_fat: u16) -> Self {
        self.sectors_per_fat = sectors_per_fat;
        self
    }

    #[must_use]
    #[inline]
    /// Set the sectors per track
    pub const fn with_sectors_per_track(mut self, sectors_per_track: u16) -> Self {
        self.sectors_per_track = sectors_per_track;
        self
    }

    #[must_use]
    #[inline]
    /// Set the number of heads
    pub const fn with_heads(mut self, heads: u16) -> Self {
        self.heads = heads;
        self
    }

    #[must_use]
    #[inline]
    /// Set the number of hidden sectors
    pub const fn with_hidden_sectors(mut self, hidden_sectors: u32) -> Self {
        self.hidden_sectors = hidden_sectors;
        self
    }

    #[must_use]
    #[inline]
    /// Set the total sectors (large count)
    ///
    /// The FAT type should be FAT32 if this is used.
    pub const fn with_total_sectors_large(mut self, total_sectors_large: u32) -> Self {
        self.total_sectors_large = total_sectors_large;
        if total_sectors_large != 0 {
            self.total_sectors = 0; // Only one of these should be non-zero
        }
        self
    }
}

/// BIOS Parameter Block (BPB) end.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct BootParamBlockEnd {
    /// Logical drive number.
    drive_number: u8,
    /// Reserved.
    _reserved: u8,
    /// Boot signature.
    boot_flag: u8,
    /// Volume serial number.
    volume_id: u32,
    /// Volume label.
    volume_label: [u8; 11],
    /// File system type.
    fs_type: [u8; 8],
}

impl Default for BootParamBlockEnd {
    fn default() -> Self {
        Self::new()
    }
}

impl BootParamBlockEnd {
    #[must_use]
    #[inline]
    /// Create a new `BootParamBlockEnd` with default values
    pub const fn new() -> Self {
        Self {
            drive_number: 0x80, // Default to hard disk
            _reserved: 0,
            boot_flag: 0x29, // Standard boot signature
            volume_id: 0,
            volume_label: *b"NO NAME    ",
            fs_type: *b"FAT     ", // Will be updated to FAT12/16/32 depending on the filesystem
        }
    }

    #[must_use]
    #[inline]
    /// Set the drive number
    pub const fn with_drive_number(mut self, drive_number: u8) -> Self {
        self.drive_number = drive_number;
        self
    }

    #[must_use]
    #[inline]
    /// Set the boot flag signature
    pub const fn with_boot_flag(mut self, boot_flag: u8) -> Self {
        self.boot_flag = boot_flag;
        self
    }

    #[must_use]
    #[inline]
    /// Set the volume ID (serial number)
    pub const fn with_volume_id(mut self, volume_id: u32) -> Self {
        self.volume_id = volume_id;
        self
    }

    #[must_use]
    #[inline]
    /// Set the volume label
    pub const fn with_volume_label(mut self, label: [u8; 11]) -> Self {
        self.volume_label = label;
        self
    }

    #[must_use]
    #[inline]
    /// Set the filesystem type
    pub const fn with_fs_type(mut self, fs_type: [u8; 8]) -> Self {
        self.fs_type = fs_type;
        self
    }
}

/// BIOS Parameter Block (BPB) for FAT12/16 file system.
#[derive(Default, Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BootParamBlock {
    bpb_start: BootParamBlockStart,
    bpb_end: BootParamBlockEnd,
}

impl BootParamBlock {
    #[must_use]
    #[inline]
    /// Returns the number of bytes per sector.
    pub const fn bytes_per_sector(&self) -> u16 {
        self.bpb_start.bytes_per_sector
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors per cluster.
    pub const fn sectors_per_cluster(&self) -> u8 {
        self.bpb_start.sectors_per_cluster
    }

    #[must_use]
    #[inline]
    /// Returns the number of reserved sectors.
    pub const fn reserved_sectors(&self) -> u16 {
        self.bpb_start.reserved_sectors
    }

    #[must_use]
    #[inline]
    /// Returns the number of FATs.
    pub const fn fat_count(&self) -> u8 {
        self.bpb_start.fat_count
    }

    #[must_use]
    #[inline]
    /// Returns the number of root directory entries.
    pub const fn root_entries(&self) -> u16 {
        self.bpb_start.root_entries
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors in the file system.
    pub fn total_sectors(&self) -> u32 {
        if self.bpb_start.total_sectors != 0 {
            u32::from(self.bpb_start.total_sectors)
        } else {
            self.bpb_start.total_sectors_large
        }
    }

    #[must_use]
    #[inline]
    /// Returns the media descriptor.
    ///
    /// Example: 0xF8 for fixed disk and 0xF0 for removable disk.
    pub const fn media_descriptor(&self) -> u8 {
        self.bpb_start.media_descriptor
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors per FAT.
    ///
    /// This value is only valid for FAT12 and FAT16 file systems.
    /// `sectors_per_fat_large` should be used for FAT32 file systems.
    pub const fn sectors_per_fat(&self) -> u16 {
        self.bpb_start.sectors_per_fat
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors per track.
    pub const fn sectors_per_track(&self) -> u16 {
        self.bpb_start.sectors_per_track
    }

    #[must_use]
    #[inline]
    /// Returns the number of heads.
    pub const fn heads(&self) -> u16 {
        self.bpb_start.heads
    }

    #[must_use]
    #[inline]
    /// Returns the number of hidden sectors.
    pub const fn hidden_sectors(&self) -> u32 {
        self.bpb_start.hidden_sectors
    }

    #[must_use]
    #[inline]
    /// Create a new `BootParamBlock` with default values for FAT12/16
    pub const fn new() -> Self {
        Self {
            bpb_start: BootParamBlockStart::new(),
            bpb_end: BootParamBlockEnd::new(),
        }
    }

    #[must_use]
    #[inline]
    /// Create a new `BootParamBlock` configured for FAT12
    pub const fn new_fat12() -> Self {
        Self::new().with_fs_type(*b"FAT12   ")
    }

    #[must_use]
    #[inline]
    /// Create a new `BootParamBlock` configured for FAT16
    pub const fn new_fat16() -> Self {
        Self::new().with_fs_type(*b"FAT16   ")
    }

    #[must_use]
    /// Calculate appropriate sectors per FAT for a FAT12 volume
    pub fn calculate_sectors_per_fat_fat12(&self) -> u16 {
        let bytes_per_sector = u32::from(self.bytes_per_sector());
        let total_sectors = self.total_sectors();
        let data_sectors = total_sectors
            - u32::from(self.reserved_sectors())
            - u32::from(self.root_entries()) * 32 / bytes_per_sector;
        let clusters = data_sectors / u32::from(self.sectors_per_cluster());

        // Each FAT entry in FAT12 is 12 bits (1.5 bytes)
        // Add some padding to ensure we have enough space
        let fat_size_bytes = (clusters * 3).div_ceil(2);
        let sectors_per_fat = fat_size_bytes.div_ceil(bytes_per_sector);

        u16::try_from(sectors_per_fat).unwrap()
    }

    #[must_use]
    /// Calculate appropriate sectors per FAT for a FAT16 volume
    pub fn calculate_sectors_per_fat_fat16(&self) -> u16 {
        let bytes_per_sector = u32::from(self.bytes_per_sector());
        let total_sectors = self.total_sectors();
        let data_sectors = total_sectors
            - u32::from(self.reserved_sectors())
            - u32::from(self.root_entries()) * 32 / bytes_per_sector;
        let clusters = data_sectors / u32::from(self.sectors_per_cluster());

        // Each FAT entry in FAT16 is 2 bytes
        // Add some padding for safety
        let fat_size_bytes = (clusters + 2) * 2;
        let sectors_per_fat = fat_size_bytes.div_ceil(bytes_per_sector);

        u16::try_from(sectors_per_fat).unwrap()
    }

    #[must_use]
    /// Set boot sector fields based on the volume size for automatic configuration
    pub fn configure_for_volume_size(mut self, volume_size_bytes: u64) -> Self {
        let bytes_per_sector = u64::from(self.bytes_per_sector());
        let total_sectors = volume_size_bytes / bytes_per_sector;

        // Configure appropriate parameters based on the volume size
        if total_sectors <= 0xFFFF {
            // Use small sector count field
            self.bpb_start.total_sectors = u16::try_from(total_sectors).unwrap();
            self.bpb_start.total_sectors_large = 0;
        } else {
            // Use large sector count field
            self.bpb_start.total_sectors = 0;
            self.bpb_start.total_sectors_large = u32::try_from(total_sectors).unwrap();
        }

        // Choose appropriate sectors per cluster based on volume size
        let sectors_per_cluster = if total_sectors < 4_085 {
            // FAT12 with very small volume: use 1 sector per cluster
            1
        } else if total_sectors < 8_190 {
            // FAT12 with small volume: use 2 sectors per cluster
            2
        } else if total_sectors < 16_450 {
            // FAT16 with small volume: use 1 sector per cluster
            1
        } else if total_sectors < 32_680 {
            // FAT16 with medium volume: use 2 sectors per cluster
            2
        } else if total_sectors < 65_525 {
            // FAT16 with larger volume: use 4 sectors per cluster
            4
        } else if total_sectors < 131_072 {
            // FAT16 with large volume: use 8 sectors per cluster
            8
        } else {
            // FAT16 with very large volume: use 16 sectors per cluster
            16
        };

        self.bpb_start.sectors_per_cluster = sectors_per_cluster;

        // Choose appropriate root directory entries
        if total_sectors < 16_450 {
            // FAT12 volumes typically use 224 root entries
            self.bpb_start.root_entries = 224;
        } else {
            // FAT16 volumes typically use 512 root entries
            self.bpb_start.root_entries = 512;
        }

        // Choose appropriate filesystem type based on the size
        if total_sectors < 4_085 {
            self = self.with_fs_type(*b"FAT12   ");
            // Calculate appropriate sectors per FAT for FAT12
            self.bpb_start.sectors_per_fat = self.calculate_sectors_per_fat_fat12();
        } else {
            self = self.with_fs_type(*b"FAT16   ");
            // Calculate appropriate sectors per FAT for FAT16
            self.bpb_start.sectors_per_fat = self.calculate_sectors_per_fat_fat16();
        }

        self
    }

    // Additional builder methods

    #[must_use]
    #[inline]
    /// Set the bytes per sector
    pub const fn with_bytes_per_sector(mut self, bytes_per_sector: u16) -> Self {
        self.bpb_start = self.bpb_start.with_bytes_per_sector(bytes_per_sector);
        self
    }

    #[must_use]
    #[inline]
    /// Set the sectors per cluster
    pub const fn with_sectors_per_cluster(mut self, sectors_per_cluster: u8) -> Self {
        self.bpb_start = self.bpb_start.with_sectors_per_cluster(sectors_per_cluster);
        self
    }

    #[must_use]
    #[inline]
    /// Set the reserved sectors
    pub const fn with_reserved_sectors(mut self, reserved_sectors: u16) -> Self {
        self.bpb_start = self.bpb_start.with_reserved_sectors(reserved_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Set the number of FATs
    pub const fn with_fat_count(mut self, fat_count: u8) -> Self {
        self.bpb_start = self.bpb_start.with_fat_count(fat_count);
        self
    }

    #[must_use]
    #[inline]
    /// Set the root entries
    pub const fn with_root_entries(mut self, root_entries: u16) -> Self {
        self.bpb_start = self.bpb_start.with_root_entries(root_entries);
        self
    }

    #[must_use]
    /// Set the total sectors
    pub fn with_total_sectors(mut self, total_sectors: u32) -> Self {
        if let Ok(total_sectors_16) = u16::try_from(total_sectors) {
            self.bpb_start = self.bpb_start.with_total_sectors(total_sectors_16);
        } else {
            self.bpb_start = self.bpb_start.with_total_sectors_large(total_sectors);
        }
        self
    }

    #[must_use]
    #[inline]
    /// Set the media descriptor
    pub const fn with_media_descriptor(mut self, media_descriptor: u8) -> Self {
        self.bpb_start = self.bpb_start.with_media_descriptor(media_descriptor);
        self
    }

    #[must_use]
    #[inline]
    /// Set the sectors per FAT
    pub const fn with_sectors_per_fat(mut self, sectors_per_fat: u16) -> Self {
        self.bpb_start = self.bpb_start.with_sectors_per_fat(sectors_per_fat);
        self
    }

    #[must_use]
    #[inline]
    /// Set the sectors per track
    pub const fn with_sectors_per_track(mut self, sectors_per_track: u16) -> Self {
        self.bpb_start = self.bpb_start.with_sectors_per_track(sectors_per_track);
        self
    }

    #[must_use]
    #[inline]
    /// Set the number of heads
    pub const fn with_heads(mut self, heads: u16) -> Self {
        self.bpb_start = self.bpb_start.with_heads(heads);
        self
    }

    #[must_use]
    #[inline]
    /// Set the number of hidden sectors
    pub const fn with_hidden_sectors(mut self, hidden_sectors: u32) -> Self {
        self.bpb_start = self.bpb_start.with_hidden_sectors(hidden_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Set the drive number
    pub const fn with_drive_number(mut self, drive_number: u8) -> Self {
        self.bpb_end = self.bpb_end.with_drive_number(drive_number);
        self
    }

    #[must_use]
    #[inline]
    /// Set the boot flag
    pub const fn with_boot_flag(mut self, boot_flag: u8) -> Self {
        self.bpb_end = self.bpb_end.with_boot_flag(boot_flag);
        self
    }

    #[must_use]
    #[inline]
    /// Set the volume ID
    pub const fn with_volume_id(mut self, volume_id: u32) -> Self {
        self.bpb_end = self.bpb_end.with_volume_id(volume_id);
        self
    }

    #[must_use]
    #[inline]
    /// Set the volume label
    pub const fn with_volume_label(mut self, label: [u8; 11]) -> Self {
        self.bpb_end = self.bpb_end.with_volume_label(label);
        self
    }

    #[must_use]
    #[inline]
    /// Set the filesystem type
    pub const fn with_fs_type(mut self, fs_type: [u8; 8]) -> Self {
        self.bpb_end = self.bpb_end.with_fs_type(fs_type);
        self
    }
}

/// BIOS Parameter Block (BPB) for FAT32 file system.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ExtendedBootParamBlock {
    // Generic FAT BPB fields.
    bpb_start: BootParamBlockStart,

    // FAT32 specific fields.
    /// Sectors per FAT.
    sectors_per_fat_large: u32,
    /// Flags.
    flags: u16,
    /// Major version.
    version_major: u8,
    /// Minor version.
    version_minor: u8,
    /// Cluster number of the root directory.
    root_cluster: u32,
    /// Sector number of the FS Information Sector.
    fs_info_sector: u16,
    /// Sector number of the backup boot sector.
    backup_boot_sector: u16,
    /// Reserved.
    _reserved: [u8; 12],

    /// Generic FAT BPB fields.
    bpb_end: BootParamBlockEnd,
}

impl Default for ExtendedBootParamBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtendedBootParamBlock {
    #[must_use]
    #[inline]
    /// Returns the number of bytes per sector.
    pub const fn bytes_per_sector(&self) -> u16 {
        self.bpb_start.bytes_per_sector
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors per cluster.
    pub const fn sectors_per_cluster(&self) -> u8 {
        self.bpb_start.sectors_per_cluster
    }

    #[must_use]
    #[inline]
    /// Returns the number of reserved sectors.
    pub const fn reserved_sectors(&self) -> u16 {
        self.bpb_start.reserved_sectors
    }

    #[must_use]
    #[inline]
    /// Returns the number of FATs.
    pub const fn fat_count(&self) -> u8 {
        self.bpb_start.fat_count
    }

    #[must_use]
    #[inline]
    /// Returns the number of root directory entries.
    pub const fn root_entries(&self) -> u16 {
        self.bpb_start.root_entries
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors in the file system.
    pub fn total_sectors(&self) -> u32 {
        if self.bpb_start.total_sectors != 0 {
            u32::from(self.bpb_start.total_sectors)
        } else {
            self.bpb_start.total_sectors_large
        }
    }

    #[must_use]
    #[inline]
    /// Returns the media descriptor.
    ///
    /// Example: 0xF8 for fixed disk and 0xF0 for removable disk.
    pub const fn media_descriptor(&self) -> u8 {
        self.bpb_start.media_descriptor
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors per FAT.
    pub const fn sectors_per_fat(&self) -> u32 {
        self.sectors_per_fat_large
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors per track.
    pub const fn sectors_per_track(&self) -> u16 {
        self.bpb_start.sectors_per_track
    }

    #[must_use]
    #[inline]
    /// Returns the number of heads.
    pub const fn heads(&self) -> u16 {
        self.bpb_start.heads
    }

    #[must_use]
    #[inline]
    /// Returns the number of hidden sectors.
    pub const fn hidden_sectors(&self) -> u32 {
        self.bpb_start.hidden_sectors
    }

    #[must_use]
    #[inline]
    /// Returns the flags.
    pub const fn flags(&self) -> u16 {
        self.flags
    }

    #[must_use]
    #[inline]
    /// Returns the major version.
    pub const fn version_major(&self) -> u8 {
        self.version_major
    }

    #[must_use]
    #[inline]
    /// Returns the minor version.
    pub const fn version_minor(&self) -> u8 {
        self.version_minor
    }

    #[must_use]
    #[inline]
    /// Returns the cluster number of the root directory.
    pub const fn root_cluster(&self) -> u32 {
        self.root_cluster
    }

    #[must_use]
    #[inline]
    /// Returns the sector number of the FS Information Sector.
    pub const fn fs_info_sector(&self) -> u16 {
        self.fs_info_sector
    }

    #[must_use]
    #[inline]
    /// Returns the sector number of the backup boot sector.
    pub const fn backup_boot_sector(&self) -> u16 {
        self.backup_boot_sector
    }

    #[must_use]
    #[inline]
    /// Returns the logical drive number.
    pub const fn drive_number(&self) -> u8 {
        self.bpb_end.drive_number
    }

    #[must_use]
    #[inline]
    /// Returns the boot signature (boot flag).
    pub const fn boot_flag(&self) -> u8 {
        self.bpb_end.boot_flag
    }

    #[must_use]
    #[inline]
    /// Returns the volume serial number.
    pub const fn volume_id(&self) -> u32 {
        self.bpb_end.volume_id
    }

    #[must_use]
    #[inline]
    /// Returns the volume label.
    pub const fn volume_label(&self) -> [u8; 11] {
        self.bpb_end.volume_label
    }

    #[must_use]
    #[inline]
    /// Returns the file system type.
    pub const fn fs_type(&self) -> &[u8] {
        &self.bpb_end.fs_type
    }

    #[must_use]
    #[inline]
    /// Create a new `ExtendedBootParamBlock` with default values
    pub const fn new() -> Self {
        Self {
            bpb_start: BootParamBlockStart::new()
                .with_reserved_sectors(32) // FAT32 typically uses more reserved sectors
                .with_root_entries(0), // Root directory is stored as a regular cluster chain
            sectors_per_fat_large: 0, // Will be calculated based on volume size
            flags: 0,                 // No active flags
            version_major: 0,         // Version 0.0
            version_minor: 0,
            root_cluster: 2,       // Root directory starts at cluster 2
            fs_info_sector: 1,     // FS info sector usually at sector 1
            backup_boot_sector: 6, // Backup boot sector usually at sector 6
            _reserved: [0; 12],
            bpb_end: BootParamBlockEnd::new().with_fs_type(*b"FAT     "),
        }
    }

    #[must_use]
    #[inline]
    /// Create a new `ExtendedBootParamBlock` with default values for FAT32
    pub const fn new_fat32() -> Self {
        Self {
            bpb_start: BootParamBlockStart::new()
                .with_reserved_sectors(32) // FAT32 typically uses more reserved sectors
                .with_root_entries(0), // Root directory is stored as a regular cluster chain
            sectors_per_fat_large: 0, // Will be calculated based on volume size
            flags: 0,                 // No active flags
            version_major: 0,         // Version 0.0
            version_minor: 0,
            root_cluster: 2,       // Root directory starts at cluster 2
            fs_info_sector: 1,     // FS info sector usually at sector 1
            backup_boot_sector: 6, // Backup boot sector usually at sector 6
            _reserved: [0; 12],
            bpb_end: BootParamBlockEnd::new().with_fs_type(*b"FAT32   "),
        }
    }

    #[must_use]
    /// Calculate appropriate sectors per FAT for a FAT32 volume
    pub fn calculate_sectors_per_fat(&self) -> u32 {
        let bytes_per_sector = u32::from(self.bytes_per_sector());
        let total_sectors = self.total_sectors();

        // Data area starts after reserved sectors and FAT areas
        let root_dir_sectors = 0; // FAT32 stores root directory as a cluster chain

        // Conservatively estimate total clusters (this is iterative in practice)
        let data_sectors = total_sectors - u32::from(self.reserved_sectors()) - root_dir_sectors;
        let sectors_per_cluster = u32::from(self.sectors_per_cluster());
        let estimated_clusters = data_sectors / sectors_per_cluster + 100; // Add padding

        // Each FAT32 entry is 4 bytes
        let fat_size_bytes = estimated_clusters * 4;
        let sectors_per_fat = fat_size_bytes.div_ceil(bytes_per_sector);

        // Add a 5% margin for safety
        sectors_per_fat + (sectors_per_fat / 20)
    }

    #[must_use]
    /// Configure the boot sector fields based on the volume size
    pub fn configure_for_volume_size(mut self, volume_size_bytes: u64) -> Self {
        let bytes_per_sector = u64::from(self.bytes_per_sector());
        let total_sectors = volume_size_bytes / bytes_per_sector;

        // FAT32 requires total_sectors_large, even for small volumes
        self.bpb_start.total_sectors = 0;
        self.bpb_start.total_sectors_large = u32::try_from(total_sectors).unwrap_or(u32::MAX);

        // Choose appropriate sectors per cluster based on volume size
        let sectors_per_cluster = if total_sectors < 532_480 {
            // Small FAT32 volume (<260MB) - 512 bytes per cluster
            1
        } else if total_sectors < 16_777_216 {
            // Medium FAT32 volume (<8GB) - 4KB per cluster
            8
        } else if total_sectors < 33_554_432 {
            // Large FAT32 volume (8-16GB) - 8KB per cluster
            16
        } else if total_sectors < 67_108_864 {
            // Very large FAT32 volume (16-32GB) - 16KB per cluster
            32
        } else {
            // Huge FAT32 volume (>32GB) - 32KB per cluster
            64
        };

        self.bpb_start.sectors_per_cluster = sectors_per_cluster;

        // Calculate appropriate sectors_per_fat
        self.sectors_per_fat_large = self.calculate_sectors_per_fat();

        self
    }

    #[must_use]
    #[inline]
    /// Set the bytes per sector
    pub const fn with_bytes_per_sector(mut self, bytes_per_sector: u16) -> Self {
        self.bpb_start = self.bpb_start.with_bytes_per_sector(bytes_per_sector);
        self
    }

    #[must_use]
    #[inline]
    /// Set the sectors per cluster
    pub const fn with_sectors_per_cluster(mut self, sectors_per_cluster: u8) -> Self {
        self.bpb_start = self.bpb_start.with_sectors_per_cluster(sectors_per_cluster);
        self
    }

    #[must_use]
    #[inline]
    /// Set the reserved sectors
    pub const fn with_reserved_sectors(mut self, reserved_sectors: u16) -> Self {
        self.bpb_start = self.bpb_start.with_reserved_sectors(reserved_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Set the number of FATs
    pub const fn with_fat_count(mut self, fat_count: u8) -> Self {
        self.bpb_start = self.bpb_start.with_fat_count(fat_count);
        self
    }

    #[must_use]
    #[inline]
    /// Set the total sectors
    pub const fn with_total_sectors(mut self, total_sectors: u32) -> Self {
        self.bpb_start.total_sectors = 0; // Must be 0 for FAT32
        self.bpb_start.total_sectors_large = total_sectors;
        self
    }

    #[must_use]
    #[inline]
    /// Set the media descriptor
    pub const fn with_media_descriptor(mut self, media_descriptor: u8) -> Self {
        self.bpb_start = self.bpb_start.with_media_descriptor(media_descriptor);
        self
    }

    #[must_use]
    #[inline]
    /// Set the sectors per FAT
    pub const fn with_sectors_per_fat(mut self, sectors_per_fat: u32) -> Self {
        self.sectors_per_fat_large = sectors_per_fat;
        self
    }

    #[must_use]
    #[inline]
    /// Set the sectors per track
    pub const fn with_sectors_per_track(mut self, sectors_per_track: u16) -> Self {
        self.bpb_start = self.bpb_start.with_sectors_per_track(sectors_per_track);
        self
    }

    #[must_use]
    #[inline]
    /// Set the number of heads
    pub const fn with_heads(mut self, heads: u16) -> Self {
        self.bpb_start = self.bpb_start.with_heads(heads);
        self
    }

    #[must_use]
    #[inline]
    /// Set the number of hidden sectors
    pub const fn with_hidden_sectors(mut self, hidden_sectors: u32) -> Self {
        self.bpb_start = self.bpb_start.with_hidden_sectors(hidden_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Set the flags
    pub const fn with_flags(mut self, flags: u16) -> Self {
        self.flags = flags;
        self
    }

    #[must_use]
    #[inline]
    /// Set the version
    pub const fn with_version(mut self, major: u8, minor: u8) -> Self {
        self.version_major = major;
        self.version_minor = minor;
        self
    }

    #[must_use]
    #[inline]
    /// Set the root cluster
    pub const fn with_root_cluster(mut self, root_cluster: u32) -> Self {
        self.root_cluster = root_cluster;
        self
    }

    #[must_use]
    #[inline]
    /// Set the FS info sector
    pub const fn with_fs_info_sector(mut self, fs_info_sector: u16) -> Self {
        self.fs_info_sector = fs_info_sector;
        self
    }

    #[must_use]
    #[inline]
    /// Set the backup boot sector
    pub const fn with_backup_boot_sector(mut self, backup_boot_sector: u16) -> Self {
        self.backup_boot_sector = backup_boot_sector;
        self
    }

    #[must_use]
    #[inline]
    /// Set the drive number
    pub const fn with_drive_number(mut self, drive_number: u8) -> Self {
        self.bpb_end = self.bpb_end.with_drive_number(drive_number);
        self
    }

    #[must_use]
    #[inline]
    /// Set the boot flag
    pub const fn with_boot_flag(mut self, boot_flag: u8) -> Self {
        self.bpb_end = self.bpb_end.with_boot_flag(boot_flag);
        self
    }

    #[must_use]
    #[inline]
    /// Set the volume ID
    pub const fn with_volume_id(mut self, volume_id: u32) -> Self {
        self.bpb_end = self.bpb_end.with_volume_id(volume_id);
        self
    }

    #[must_use]
    #[inline]
    /// Set the volume label
    pub const fn with_volume_label(mut self, label: [u8; 11]) -> Self {
        self.bpb_end = self.bpb_end.with_volume_label(label);
        self
    }

    #[must_use]
    #[inline]
    /// Set the filesystem type
    pub const fn with_fs_type(mut self, fs_type: [u8; 8]) -> Self {
        self.bpb_end = self.bpb_end.with_fs_type(fs_type);
        self
    }
}

impl BootParamBlock {
    #[must_use]
    #[inline]
    pub const fn is_fat32(&self) -> bool {
        // This field must be zero on FAT32 file systems
        // and non-zero on FAT12 and FAT16 file systems.
        self.sectors_per_fat() == 0
    }

    #[must_use]
    #[inline]
    /// Returns the number of bytes per cluster.
    pub fn bytes_per_cluster(&self) -> u32 {
        u32::from(self.bytes_per_sector()) * u32::from(self.sectors_per_cluster())
    }

    #[must_use]
    pub fn validate(&self) -> bool {
        /// Maximum bytes per cluster for maximum compatibility.
        const MAX_BYTES_PER_CLUSTER: u32 = 32 * 1024; // 32 KiB
        /// Maximum number of supported FAT
        const MAX_FAT_COUNT: u8 = 2;

        // TODO: Check version?

        // Check bytes per sector
        if !self.bytes_per_sector().is_power_of_two()
            || self.bytes_per_sector() < 512
            || self.bytes_per_sector() > 4096
        {
            return false;
        }

        // Check sectors per cluster
        if !self.sectors_per_cluster().is_power_of_two()
            || (u32::from(self.bytes_per_sector()) * u32::from(self.sectors_per_cluster())
                > MAX_BYTES_PER_CLUSTER)
        {
            return false;
        }

        // Check reserved sectors
        if self.reserved_sectors() == 0 {
            return false;
        }

        // Check FAT count
        if self.fat_count() == 0 || self.fat_count() > MAX_FAT_COUNT {
            return false;
        }

        // Check root entries
        if usize::from(self.root_entries()) * super::dirent::DIR_ENTRY_SIZE
            % usize::from(self.bytes_per_sector())
            != 0
        {
            return false;
        }

        // Check total sectors
        if (self.bpb_start.total_sectors != 0 && self.bpb_start.total_sectors_large != 0)
            || (self.bpb_start.total_sectors == 0 && self.bpb_start.total_sectors_large == 0)
        {
            return false;
        }

        // Check sectors per fat
        if self.sectors_per_fat() == 0 {
            return false;
        }

        // TODO: Check clusters

        true
    }
}

impl ExtendedBootParamBlock {
    #[must_use]
    #[inline]
    pub const fn is_fat32(&self) -> bool {
        true
    }

    #[must_use]
    #[inline]
    /// Returns the number of bytes per cluster.
    pub fn bytes_per_cluster(&self) -> u32 {
        u32::from(self.bytes_per_sector()) * u32::from(self.sectors_per_cluster())
    }

    #[must_use]
    pub fn validate(&self) -> bool {
        /// Maximum bytes per cluster for maximum compatibility.
        const MAX_BYTES_PER_CLUSTER: u32 = 32 * 1024; // 32 KiB
        /// Maximum number of supported FAT
        const MAX_FAT_COUNT: u8 = 2;

        // TODO: Check version?

        // Check bytes per sector
        if !self.bytes_per_sector().is_power_of_two()
            || self.bytes_per_sector() < 512
            || self.bytes_per_sector() > 4096
        {
            return false;
        }

        // Check sectors per cluster
        if !self.sectors_per_cluster().is_power_of_two()
            || (u32::from(self.bytes_per_sector()) * u32::from(self.sectors_per_cluster())
                > MAX_BYTES_PER_CLUSTER)
        {
            return false;
        }

        // Check reserved sectors
        if self.reserved_sectors() == 0
            || self.backup_boot_sector >= self.reserved_sectors()
            || self.fs_info_sector >= self.reserved_sectors()
        {
            return false;
        }

        // Check FAT count
        if self.fat_count() == 0 || self.fat_count() > MAX_FAT_COUNT {
            return false;
        }

        // Check root entries
        if self.root_entries() != 0
            || (usize::from(self.root_entries()) * super::dirent::DIR_ENTRY_SIZE
                % usize::from(self.bytes_per_sector())
                != 0)
        {
            return false;
        }

        // Check total sectors
        if self.bpb_start.total_sectors != 0 || self.bpb_start.total_sectors_large == 0 {
            return false;
        }

        // Check sectors per fat
        if self.sectors_per_fat() == 0 {
            return false;
        }

        // TODO: Check clusters

        true
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BootSector {
    /// Jump instruction.
    boot_jump: [u8; 3],
    /// OEM name.
    oem_name: [u8; 8],
    /// Boot Parameter Block (BPB).
    bpb: BootParamBlock,
    boot_code: [u8; 448],
    /// Boot signature.
    boot_signature: [u8; 2],
}
static_assert!(
    size_of::<BootSector>() == 512,
    "BootSector size is not 512 bytes"
);

impl Default for BootSector {
    fn default() -> Self {
        Self::new()
    }
}

impl BootSector {
    #[must_use]
    #[inline]
    /// Create a new default `BootSector` for FAT12/16
    pub const fn new() -> Self {
        Self {
            boot_jump: [0xEB, 0x3C, 0x90], // Standard boot jump instruction (jmp short 0x3E; nop)
            oem_name: *b"BESKAROS",        // OEM name (8 chars)
            bpb: BootParamBlock::new(),    // Use default BPB values
            boot_code: [0; 448],           // Empty boot code
            boot_signature: [0x55, 0xAA],  // Boot signature (always 0x55AA)
        }
    }

    #[must_use]
    #[inline]
    /// Create a new `BootSector` for FAT12
    pub const fn new_fat12() -> Self {
        Self {
            boot_jump: [0xEB, 0x3C, 0x90],
            oem_name: *b"BESKAROS",
            bpb: BootParamBlock::new_fat12(),
            boot_code: [0; 448],
            boot_signature: [0x55, 0xAA],
        }
    }

    #[must_use]
    #[inline]
    /// Create a new `BootSector` for FAT16
    pub const fn new_fat16() -> Self {
        Self {
            boot_jump: [0xEB, 0x3C, 0x90],
            oem_name: *b"BESKAROS",
            bpb: BootParamBlock::new_fat16(),
            boot_code: [0; 448],
            boot_signature: [0x55, 0xAA],
        }
    }

    #[must_use]
    #[inline]
    /// Configure the boot sector for a specific volume size
    pub fn configure_for_volume_size(mut self, volume_size_bytes: u64) -> Self {
        self.bpb = self.bpb.configure_for_volume_size(volume_size_bytes);
        self
    }

    #[must_use]
    #[inline]
    /// Set the boot jump instruction
    pub const fn with_boot_jump(mut self, boot_jump: [u8; 3]) -> Self {
        self.boot_jump = boot_jump;
        self
    }

    #[must_use]
    #[inline]
    /// Set the OEM name
    pub const fn with_oem_name(mut self, oem_name: [u8; 8]) -> Self {
        self.oem_name = oem_name;
        self
    }

    #[must_use]
    #[inline]
    /// Set the BPB
    pub const fn with_bpb(mut self, bpb: BootParamBlock) -> Self {
        self.bpb = bpb;
        self
    }

    #[must_use]
    #[inline]
    /// Set the boot code
    pub fn with_boot_code(mut self, boot_code: &[u8]) -> Self {
        assert!(
            boot_code.len() <= 448,
            "Boot code must be 448 bytes or less"
        );
        self.boot_code[..boot_code.len()].copy_from_slice(boot_code);
        self.boot_code[boot_code.len()..].fill(0);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_bytes_per_sector(mut self, bytes_per_sector: u16) -> Self {
        self.bpb = self.bpb.with_bytes_per_sector(bytes_per_sector);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_sectors_per_cluster(mut self, sectors_per_cluster: u8) -> Self {
        self.bpb = self.bpb.with_sectors_per_cluster(sectors_per_cluster);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_reserved_sectors(mut self, reserved_sectors: u16) -> Self {
        self.bpb = self.bpb.with_reserved_sectors(reserved_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_fat_count(mut self, fat_count: u8) -> Self {
        self.bpb = self.bpb.with_fat_count(fat_count);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_root_entries(mut self, root_entries: u16) -> Self {
        self.bpb = self.bpb.with_root_entries(root_entries);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub fn with_total_sectors(mut self, total_sectors: u32) -> Self {
        self.bpb = self.bpb.with_total_sectors(total_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_media_descriptor(mut self, media_descriptor: u8) -> Self {
        self.bpb = self.bpb.with_media_descriptor(media_descriptor);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_sectors_per_fat(mut self, sectors_per_fat: u16) -> Self {
        self.bpb = self.bpb.with_sectors_per_fat(sectors_per_fat);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_sectors_per_track(mut self, sectors_per_track: u16) -> Self {
        self.bpb = self.bpb.with_sectors_per_track(sectors_per_track);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_heads(mut self, heads: u16) -> Self {
        self.bpb = self.bpb.with_heads(heads);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_hidden_sectors(mut self, hidden_sectors: u32) -> Self {
        self.bpb = self.bpb.with_hidden_sectors(hidden_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_drive_number(mut self, drive_number: u8) -> Self {
        self.bpb = self.bpb.with_drive_number(drive_number);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_boot_flag(mut self, boot_flag: u8) -> Self {
        self.bpb = self.bpb.with_boot_flag(boot_flag);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_volume_id(mut self, volume_id: u32) -> Self {
        self.bpb = self.bpb.with_volume_id(volume_id);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_volume_label(mut self, label: [u8; 11]) -> Self {
        self.bpb = self.bpb.with_volume_label(label);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_fs_type(mut self, fs_type: [u8; 8]) -> Self {
        self.bpb = self.bpb.with_fs_type(fs_type);
        self
    }

    #[must_use]
    #[inline]
    /// Validates the boot sector
    pub fn validate(&self) -> bool {
        // Check boot signature
        if self.boot_signature != [0x55, 0xAA] {
            return false;
        }

        // Check BPB
        self.bpb.validate()
    }

    #[must_use]
    #[inline]
    /// Access the underlying BPB
    pub const fn bpb(&self) -> &BootParamBlock {
        &self.bpb
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ExtendedBootSector {
    /// Jump instruction.
    boot_jump: [u8; 3],
    /// OEM name.
    oem_name: [u8; 8],
    /// Boot Parameter Block (BPB).
    bpb: ExtendedBootParamBlock,
    boot_code: [u8; 420],
    /// Boot signature.
    boot_signature: [u8; 2],
}
static_assert!(
    size_of::<ExtendedBootSector>() == 512,
    "ExtendedBootSector size is not 512 bytes"
);

impl Default for ExtendedBootSector {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtendedBootSector {
    #[must_use]
    #[inline]
    /// Create a new default `ExtendedBootSector`
    pub const fn new() -> Self {
        Self {
            boot_jump: [0xEB, 0x58, 0x90], // Standard boot jump instruction for FAT32
            oem_name: *b"BESKAROS",        // OEM name (8 chars)
            bpb: ExtendedBootParamBlock::new(), // Use default FAT32 BPB values
            boot_code: [0; 420],           // Empty boot code
            boot_signature: [0x55, 0xAA],  // Boot signature (always 0x55AA)
        }
    }

    #[must_use]
    #[inline]
    /// Create a new `ExtendedBootSector` for FAT32
    pub const fn new_fat32() -> Self {
        Self {
            boot_jump: [0xEB, 0x58, 0x90], // Standard boot jump instruction for FAT32
            oem_name: *b"BESKAROS",        // OEM name (8 chars)
            bpb: ExtendedBootParamBlock::new_fat32(), // Use default FAT32 BPB values
            boot_code: [0; 420],           // Empty boot code
            boot_signature: [0x55, 0xAA],  // Boot signature (always 0x55AA)
        }
    }

    #[must_use]
    #[inline]
    /// Configure the boot sector for a specific volume size
    pub fn configure_for_volume_size(mut self, volume_size_bytes: u64) -> Self {
        self.bpb = self.bpb.configure_for_volume_size(volume_size_bytes);
        self
    }

    #[must_use]
    #[inline]
    /// Set the boot jump instruction
    pub const fn with_boot_jump(mut self, boot_jump: [u8; 3]) -> Self {
        self.boot_jump = boot_jump;
        self
    }

    #[must_use]
    #[inline]
    /// Set the OEM name
    pub const fn with_oem_name(mut self, oem_name: [u8; 8]) -> Self {
        self.oem_name = oem_name;
        self
    }

    #[must_use]
    #[inline]
    /// Set the BPB
    pub const fn with_bpb(mut self, bpb: ExtendedBootParamBlock) -> Self {
        self.bpb = bpb;
        self
    }

    #[must_use]
    #[inline]
    /// Set the boot code
    pub fn with_boot_code(mut self, boot_code: &[u8]) -> Self {
        assert!(
            boot_code.len() <= 420,
            "Boot code must be 420 bytes or less"
        );
        self.boot_code[..boot_code.len()].copy_from_slice(boot_code);
        self.boot_code[boot_code.len()..].fill(0);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_bytes_per_sector(mut self, bytes_per_sector: u16) -> Self {
        self.bpb = self.bpb.with_bytes_per_sector(bytes_per_sector);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_sectors_per_cluster(mut self, sectors_per_cluster: u8) -> Self {
        self.bpb = self.bpb.with_sectors_per_cluster(sectors_per_cluster);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_reserved_sectors(mut self, reserved_sectors: u16) -> Self {
        self.bpb = self.bpb.with_reserved_sectors(reserved_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_fat_count(mut self, fat_count: u8) -> Self {
        self.bpb = self.bpb.with_fat_count(fat_count);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_total_sectors(mut self, total_sectors: u32) -> Self {
        self.bpb = self.bpb.with_total_sectors(total_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_media_descriptor(mut self, media_descriptor: u8) -> Self {
        self.bpb = self.bpb.with_media_descriptor(media_descriptor);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_sectors_per_fat(mut self, sectors_per_fat: u32) -> Self {
        self.bpb = self.bpb.with_sectors_per_fat(sectors_per_fat);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_sectors_per_track(mut self, sectors_per_track: u16) -> Self {
        self.bpb = self.bpb.with_sectors_per_track(sectors_per_track);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_heads(mut self, heads: u16) -> Self {
        self.bpb = self.bpb.with_heads(heads);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_hidden_sectors(mut self, hidden_sectors: u32) -> Self {
        self.bpb = self.bpb.with_hidden_sectors(hidden_sectors);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_flags(mut self, flags: u16) -> Self {
        self.bpb = self.bpb.with_flags(flags);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_version(mut self, major: u8, minor: u8) -> Self {
        self.bpb = self.bpb.with_version(major, minor);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_root_cluster(mut self, root_cluster: u32) -> Self {
        self.bpb = self.bpb.with_root_cluster(root_cluster);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_fs_info_sector(mut self, fs_info_sector: u16) -> Self {
        self.bpb = self.bpb.with_fs_info_sector(fs_info_sector);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_backup_boot_sector(mut self, backup_boot_sector: u16) -> Self {
        self.bpb = self.bpb.with_backup_boot_sector(backup_boot_sector);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_drive_number(mut self, drive_number: u8) -> Self {
        self.bpb = self.bpb.with_drive_number(drive_number);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_boot_flag(mut self, boot_flag: u8) -> Self {
        self.bpb = self.bpb.with_boot_flag(boot_flag);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_volume_id(mut self, volume_id: u32) -> Self {
        self.bpb = self.bpb.with_volume_id(volume_id);
        self
    }

    #[must_use]
    #[inline]
    /// Forward method to BPB
    pub const fn with_volume_label(mut self, label: [u8; 11]) -> Self {
        self.bpb = self.bpb.with_volume_label(label);
        self
    }

    #[must_use]
    #[inline]
    /// Validates the boot sector
    pub fn validate(&self) -> bool {
        // Check boot signature
        if self.boot_signature != [0x55, 0xAA] {
            return false;
        }

        // Check BPB
        self.bpb.validate()
    }

    #[must_use]
    #[inline]
    /// Access the underlying BPB
    pub const fn bpb(&self) -> &ExtendedBootParamBlock {
        &self.bpb
    }
}

pub type BootSectorUnion = super::FatUnion<BootSector, BootSector, ExtendedBootSector>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_param_block() {
        let mut bpb = BootParamBlock {
            bpb_start: BootParamBlockStart {
                bytes_per_sector: 512,
                sectors_per_cluster: 1,
                reserved_sectors: 1,
                fat_count: 2,
                root_entries: 16,
                total_sectors: 2048,
                media_descriptor: 0xF8,
                sectors_per_fat: 9,
                sectors_per_track: 18,
                heads: 2,
                hidden_sectors: 0,
                total_sectors_large: 0,
            },
            bpb_end: BootParamBlockEnd {
                drive_number: 0x80,
                _reserved: 0,
                boot_flag: 0x29,
                volume_id: 0x12345678,
                volume_label: *b"NO NAME    ",
                fs_type: *b"FAT16   ",
            },
        };

        // Test accessors
        assert_eq!(bpb.bytes_per_sector(), 512);
        assert_eq!(bpb.sectors_per_cluster(), 1);
        assert_eq!(bpb.reserved_sectors(), 1);
        assert_eq!(bpb.fat_count(), 2);
        assert_eq!(bpb.root_entries(), 16);
        assert_eq!(bpb.total_sectors(), 2048);
        assert_eq!(bpb.media_descriptor(), 0xF8);
        assert_eq!(bpb.sectors_per_fat(), 9);
        assert_eq!(bpb.sectors_per_track(), 18);
        assert_eq!(bpb.heads(), 2);
        assert_eq!(bpb.hidden_sectors(), 0);

        // Test bytes per cluster
        assert_eq!(bpb.bytes_per_cluster(), 512);

        // Test is_fat32
        assert!(!bpb.is_fat32());

        // Test validation
        assert!(bpb.validate());

        // Test invalid values
        bpb.bpb_start.bytes_per_sector = 513; // Not a power of 2
        assert!(!bpb.validate());

        bpb.bpb_start.bytes_per_sector = 512;
        bpb.bpb_start.sectors_per_cluster = 0;
        assert!(!bpb.validate());

        bpb.bpb_start.sectors_per_cluster = 1;
        bpb.bpb_start.reserved_sectors = 0;
        assert!(!bpb.validate());

        bpb.bpb_start.reserved_sectors = 1;
        bpb.bpb_start.fat_count = 0;
        assert!(!bpb.validate());

        bpb.bpb_start.fat_count = 3; // More than maximum supported
        assert!(!bpb.validate());

        bpb.bpb_start.fat_count = 2;
        bpb.bpb_start.total_sectors = 0;
        assert!(!bpb.validate());
    }

    #[test]
    fn test_extended_boot_param_block() {
        let mut ebpb = ExtendedBootParamBlock {
            bpb_start: BootParamBlockStart {
                bytes_per_sector: 512,
                sectors_per_cluster: 8,
                reserved_sectors: 32,
                fat_count: 2,
                root_entries: 0,  // Must be 0 for FAT32
                total_sectors: 0, // Must use total_sectors_large for FAT32
                media_descriptor: 0xF8,
                sectors_per_fat: 0, // Must be 0 for FAT32
                sectors_per_track: 63,
                heads: 255,
                hidden_sectors: 0,
                total_sectors_large: 0x00100000, // 1 million sectors = ~500 MB
            },
            sectors_per_fat_large: 1000,
            flags: 0,
            version_major: 0,
            version_minor: 0,
            root_cluster: 2,
            fs_info_sector: 1,
            backup_boot_sector: 6,
            _reserved: [0; 12],
            bpb_end: BootParamBlockEnd {
                drive_number: 0x80,
                _reserved: 0,
                boot_flag: 0x29,
                volume_id: 0x12345678,
                volume_label: *b"NO NAME    ",
                fs_type: *b"FAT32   ",
            },
        };

        // Test accessors
        assert_eq!(ebpb.bytes_per_sector(), 512);
        assert_eq!(ebpb.sectors_per_cluster(), 8);
        assert_eq!(ebpb.reserved_sectors(), 32);
        assert_eq!(ebpb.fat_count(), 2);
        assert_eq!(ebpb.root_entries(), 0);
        assert_eq!(ebpb.total_sectors(), 0x00100000);
        assert_eq!(ebpb.media_descriptor(), 0xF8);
        assert_eq!(ebpb.sectors_per_fat(), 1000);
        assert_eq!(ebpb.sectors_per_track(), 63);
        assert_eq!(ebpb.heads(), 255);
        assert_eq!(ebpb.hidden_sectors(), 0);
        assert_eq!(ebpb.flags(), 0);
        assert_eq!(ebpb.version_major(), 0);
        assert_eq!(ebpb.version_minor(), 0);
        assert_eq!(ebpb.root_cluster(), 2);
        assert_eq!(ebpb.fs_info_sector(), 1);
        assert_eq!(ebpb.backup_boot_sector(), 6);
        assert_eq!(ebpb.drive_number(), 0x80);
        assert_eq!(ebpb.boot_flag(), 0x29);
        assert_eq!(ebpb.volume_id(), 0x12345678);
        assert_eq!(ebpb.volume_label(), *b"NO NAME    ");
        assert_eq!(ebpb.fs_type(), b"FAT32   ");

        // Test bytes per cluster
        assert_eq!(ebpb.bytes_per_cluster(), 4096); // 512 * 8

        // Test is_fat32
        assert!(ebpb.is_fat32());

        // Test validation
        assert!(ebpb.validate());

        // Test invalid values
        ebpb.bpb_start.bytes_per_sector = 513; // Not a power of 2
        assert!(!ebpb.validate());

        ebpb.bpb_start.bytes_per_sector = 512;
        ebpb.bpb_start.sectors_per_cluster = 0;
        assert!(!ebpb.validate());

        ebpb.bpb_start.sectors_per_cluster = 8;
        ebpb.bpb_start.reserved_sectors = 0;
        assert!(!ebpb.validate());

        ebpb.bpb_start.reserved_sectors = 32;
        ebpb.backup_boot_sector = 33; // Beyond reserved sectors
        assert!(!ebpb.validate());

        ebpb.backup_boot_sector = 6;
        ebpb.bpb_start.fat_count = 0;
        assert!(!ebpb.validate());

        ebpb.bpb_start.fat_count = 3; // More than maximum supported
        assert!(!ebpb.validate());

        ebpb.bpb_start.fat_count = 2;
        ebpb.bpb_start.root_entries = 512; // Must be 0 for FAT32
        assert!(!ebpb.validate());

        ebpb.bpb_start.root_entries = 0;
        ebpb.bpb_start.total_sectors = 1000; // Must be 0 for FAT32
        assert!(!ebpb.validate());

        ebpb.bpb_start.total_sectors = 0;
        ebpb.sectors_per_fat_large = 0; // Must be non-zero
        assert!(!ebpb.validate());
    }
}
