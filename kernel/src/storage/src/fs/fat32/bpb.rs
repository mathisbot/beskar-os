/// BIOS Parameter Block (BPB) for FAT32 file system.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Bpb {
    // Generic FAT BPB fields.
    /// Jump instruction.
    boot_code: [u8; 3],
    /// OEM name.
    oem_name: [u8; 8],
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
    _sectors_per_fat: u16,
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

    // FAT32 specific fields.
    /// Sectors per FAT.
    sectors_per_fat: u32,
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
    _reserved0: [u8; 12],
    /// Logical drive number.
    drive_number: u8,
    /// Reserved.
    _reserved1: u8,
    /// Boot signature.
    boot_flag: u8,
    /// Volume serial number.
    volume_id: u32,
    /// Volume label.
    volume_label: [u8; 11],
    /// File system type.
    fs_type: [u8; 8],
}

impl Bpb {
    #[must_use]
    #[inline]
    /// Returns the number of sectors in the file system.
    pub fn total_sectors(&self) -> u32 {
        if self.total_sectors != 0 {
            u32::from(self.total_sectors)
        } else {
            self.total_sectors_large
        }
    }

    #[must_use]
    #[inline]
    /// Returns the number of bytes per cluster.
    pub fn bytes_per_cluster(&self) -> u32 {
        u32::from(self.bytes_per_sector) * u32::from(self.sectors_per_cluster)
    }

    #[must_use]
    #[inline]
    /// Returns the number of bytes per sector.
    pub const fn bytes_per_sector(&self) -> u16 {
        self.bytes_per_sector
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors per cluster.
    pub const fn sectors_per_cluster(&self) -> u8 {
        self.sectors_per_cluster
    }

    #[must_use]
    #[inline]
    /// Returns the number of sectors per FAT.
    pub const fn sectors_per_fat(&self) -> u32 {
        self.sectors_per_fat
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
    /// Returns the volume serial number.
    pub const fn volume_id(&self) -> u32 {
        self.volume_id
    }

    #[must_use]
    #[inline]
    /// Returns the volume label.
    pub const fn volume_label(&self) -> [u8; 11] {
        self.volume_label
    }

    #[must_use]
    #[inline]
    /// Returns the file system type.
    pub const fn fs_type(&self) -> &[u8] {
        &self.fs_type
    }

    #[must_use]
    pub fn validate(&self) -> bool {
        /// Maximum bytes per cluster for maximum compatibility.
        const MAX_BYTES_PER_CLUSTER: u32 = 32 * 1024; // 32 KiB
        /// Maximum number of supported FAT
        const MAX_FAT_COUNT: u8 = 2;

        // TODO: Check version?

        // Check bytes per sector
        if !self.bytes_per_sector.is_power_of_two()
            || self.bytes_per_sector < 512
            || self.bytes_per_sector > 4096
        {
            return false;
        }

        // Check sectors per cluster
        if !self.sectors_per_cluster.is_power_of_two()
            || (u32::from(self.bytes_per_sector) * u32::from(self.sectors_per_cluster)
                > MAX_BYTES_PER_CLUSTER)
        {
            return false;
        }

        // Check reserved sectors
        if self.reserved_sectors == 0
            || self.backup_boot_sector >= self.reserved_sectors
            || self.fs_info_sector >= self.reserved_sectors
        {
            return false;
        }

        // Check FAT count
        if self.fat_count == 0 || self.fat_count > MAX_FAT_COUNT {
            return false;
        }

        // Check root entries
        if self.root_entries != 0
            || (usize::from(self.root_entries) * super::DIR_ENTRY_SIZE
                % usize::from(self.bytes_per_sector)
                != 0)
        {
            return false;
        }

        // Check total sectors
        if self.total_sectors != 0 || self.total_sectors_large == 0 {
            return false;
        }

        // Check sectors per fat
        if self.sectors_per_fat == 0 {
            return false;
        }

        // TODO: Check clusters

        true
    }
}
