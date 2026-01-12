//! AHCI Command Management
//!
//! Command builders and command queue management for AHCI.

use super::fis::{AtaCommand, FisH2D};

/// Command structure to be sent to the device
pub struct AhciCommand {
    pub fis: FisH2D,
}

impl AhciCommand {
    #[must_use]
    #[inline]
    /// Create a new ATA command
    pub const fn new() -> Self {
        Self { fis: FisH2D::new() }
    }

    #[must_use]
    #[inline]
    /// Build an Identify Device command
    pub const fn identify_device() -> Self {
        let mut cmd = Self::new();
        cmd.fis.command = AtaCommand::IdentifyDevice as u8;
        cmd
    }

    #[must_use]
    #[inline]
    /// Build a Read DMA Extended command
    pub const fn read_dma_ext(lba: u64, count: u16) -> Self {
        let mut cmd = Self::new();
        cmd.fis.command = AtaCommand::ReadDmaEx as u8;
        cmd.fis.set_lba(lba);
        cmd.fis.set_count(count);
        cmd
    }

    #[must_use]
    #[inline]
    /// Build a Write DMA Extended command
    pub const fn write_dma_ext(lba: u64, count: u16) -> Self {
        let mut cmd = Self::new();
        cmd.fis.command = AtaCommand::WriteDmaEx as u8;
        cmd.fis.set_lba(lba);
        cmd.fis.set_count(count);
        cmd
    }

    #[must_use]
    #[inline]
    /// Build a Read Sector Extended command
    pub const fn read_sector_ext(lba: u64, count: u16) -> Self {
        let mut cmd = Self::new();
        cmd.fis.command = AtaCommand::ReadSectorEx as u8;
        cmd.fis.set_lba(lba);
        cmd.fis.set_count(count);
        cmd
    }

    #[must_use]
    #[inline]
    /// Build a Write Sector Extended command
    pub const fn write_sector_ext(lba: u64, count: u16) -> Self {
        let mut cmd = Self::new();
        cmd.fis.command = AtaCommand::WriteSectorEx as u8;
        cmd.fis.set_lba(lba);
        cmd.fis.set_count(count);
        cmd
    }

    #[must_use]
    #[inline]
    /// Get the underlying FIS structure
    pub const fn fis(&self) -> &FisH2D {
        &self.fis
    }

    #[must_use]
    #[inline]
    /// Get mutable reference to FIS for advanced configuration
    pub const fn fis_mut(&mut self) -> &mut FisH2D {
        &mut self.fis
    }
}
