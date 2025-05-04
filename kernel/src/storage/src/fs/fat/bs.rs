use beskar_core::static_assert;

/// BIOS Parameter Block (BPB) start.
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

/// BIOS Parameter Block (BPB) end.
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

/// BIOS Parameter Block (BPB) for FAT12/16 file system.
#[derive(Debug, Clone, Copy)]
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
        if self.root_entries() != 0
            || (usize::from(self.root_entries()) * super::dirent::DIR_ENTRY_SIZE
                % usize::from(self.bytes_per_sector())
                != 0)
        {
            return false;
        }

        // Check total sectors
        if self.bpb_start.total_sectors == 0 {
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
    #[allow(clippy::unused_self)]
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

pub type BootSectorUnion = super::FatUnion<BootSector, BootSector, ExtendedBootSector>;
