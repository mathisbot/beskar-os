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
