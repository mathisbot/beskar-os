use noto_sans_mono_bitmap::{
    FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width,
};

const CHAR_HEIGHT_INTERNAL: RasterHeight = RasterHeight::Size20;
#[expect(clippy::cast_possible_truncation, reason = "No truncation here")]
pub const CHAR_HEIGHT: u16 = CHAR_HEIGHT_INTERNAL.val() as _;
#[expect(clippy::cast_possible_truncation, reason = "No truncation here")]
pub const CHAR_WIDTH: u16 = get_raster_width(FontWeight::Regular, CHAR_HEIGHT_INTERNAL) as _;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_raster_backed() {
        for c in '\0'..='~' {
            let _ = get_raster_backed(c);
        }
        let _ = get_raster_backed('中');
        let _ = get_raster_backed('\u{10FFFF}');
    }
}
