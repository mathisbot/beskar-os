use noto_sans_mono_bitmap::{
    FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width,
};

const CHAR_HEIGHT_INTERNAL: RasterHeight = RasterHeight::Size20;
pub const CHAR_HEIGHT: usize = CHAR_HEIGHT_INTERNAL.val();
pub const CHAR_WIDTH: usize = get_raster_width(FontWeight::Regular, CHAR_HEIGHT_INTERNAL);
const BACKUP_CHAR: char = '�';

#[must_use]
#[inline]
/// Returns the raster of the given char,
/// backing up to a default char if the given char is not available.
pub fn get_raster_backed(c: char) -> RasterizedChar {
    get_raster(c, FontWeight::Regular, CHAR_HEIGHT_INTERNAL).unwrap_or_else(|| unsafe {
        get_raster(BACKUP_CHAR, FontWeight::Regular, CHAR_HEIGHT_INTERNAL).unwrap_unchecked()
    })
}
