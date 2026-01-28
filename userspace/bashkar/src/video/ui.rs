use super::screen;
use alloc::string::String;
use ascii_ui::{AsciiCanvas, BoxStyle, CharRect, Theme};
use beskar_core::video::PixelComponents;
use hyperdrive::once::Once;

pub const PRIMARY_GREEN: PixelComponents = PixelComponents::GREEN;
pub const SHADOW_GREEN: PixelComponents = PixelComponents::new(0x00, 0xAA, 0x00);

pub const TEXT_COLOR: PixelComponents = PixelComponents::WHITE;
pub const BACKGROUND_COLOR: PixelComponents = PixelComponents::BLACK;

#[derive(Clone, Copy, Debug)]
pub struct UiLayout {
    /// Inner console top in character rows (excludes border)
    pub text_start_row: u16,
    /// Inner console left in character columns
    pub inner_left_col: u16,
    /// Inner console width in character columns
    pub inner_width_cols: u16,
    /// Inner console height in character rows
    pub inner_height_rows: u16,
    /// Cell width in pixels for char grid
    pub cell_width_px: u16,
    /// Cell height in pixels for char grid
    pub cell_height_px: u16,
}

static LAYOUT: Once<UiLayout> = Once::uninit();

#[must_use]
/// Returns the computed UI layout.
///
/// # Panics
///
/// Panics if the UI has not been initialized yet via `draw()`.
pub fn layout() -> &'static UiLayout {
    LAYOUT.get().expect("UI not initialized")
}

/// Clamps a string to the available column width.
#[inline]
fn clamp_str(s: &str, max_cols: u16) -> String {
    s.chars().take(max_cols as usize).collect()
}

/// Draw the UI and initialize the layout.
pub fn draw() {
    screen::with_screen(|fb| {
        let info = *fb.info();
        let theme = Theme {
            foreground: PRIMARY_GREEN,
            background: BACKGROUND_COLOR,
        };
        let mut view = fb.view();
        let mut canvas = AsciiCanvas::new(info, view.pixels_mut(), theme);

        canvas.clear(BACKGROUND_COLOR);
        canvas.set_color(PRIMARY_GREEN);

        let cols = canvas.cols();
        let rows = canvas.rows();

        let header_row = 1;
        let header_text = clamp_str("[ TERMINAL ]", cols);
        canvas.write_line_centered(header_row, &header_text);

        canvas.set_color(SHADOW_GREEN);
        if rows > header_row + 1 {
            let status = clamp_str("DS-1 ORBITAL BATTLE STATION // IMPERIAL NETWORK", cols);
            canvas.write_line_centered(header_row + 1, &status);
        }

        canvas.set_color(PRIMARY_GREEN);
        if rows > header_row + 2 {
            canvas.h_rule(header_row + 2, '=');
        }

        // Conditionally render the Death Star art if space allows
        let art_start = header_row + 4;
        let art_end = if rows > art_start + DS_ART_HEIGHT && cols >= DS_ART_WIDTH {
            let h = draw_death_star(&mut canvas, art_start);
            art_start + h
        } else {
            art_start
        };

        let status_row = art_end + 1;
        canvas.set_color(SHADOW_GREEN);
        if status_row < rows.saturating_sub(3) {
            let status2 = clamp_str("IMPERIAL ACCESS CHANNEL // LEVEL 3 CLEARANCE", cols);
            canvas.write_line_centered(status_row, &status2);
        }
        canvas.set_color(PRIMARY_GREEN);

        // Console starts after status line with padding
        let mut console_top = status_row + 2;

        // Ensure console has minimum height
        let min_console_inner = 3u16;
        // If not enough space, pull console_top upwards to fit minimum console
        if console_top + (min_console_inner + 2) > rows {
            // +2 for box borders if any
            let needed = min_console_inner + 2;
            console_top = rows.saturating_sub(needed);
            if console_top <= header_row + 2 {
                console_top = header_row + 3;
            }
        }

        let margin = if cols >= 10 { 2 } else { 1 };
        let panel_w = cols.saturating_sub(margin * 2);
        let panel_h = rows.saturating_sub(console_top + margin);
        let console_rect = CharRect {
            x: margin,
            y: console_top,
            width: panel_w,
            height: panel_h,
        };

        let can_box = console_rect.width >= 4 && console_rect.height >= 3;
        if can_box {
            canvas.stroke_box(console_rect, &BoxStyle::classic());
        }

        let inner = if can_box {
            console_rect.inset(1)
        } else {
            console_rect
        };
        let text_start_row = inner.y; // inner top row
        let inner_h = if inner.height == 0 { 1 } else { inner.height };
        let inner_w = if inner.width == 0 { 1 } else { inner.width };
        LAYOUT.call_once(|| UiLayout {
            text_start_row,
            inner_left_col: inner.x,
            inner_width_cols: inner_w,
            inner_height_rows: inner_h,
            cell_width_px: canvas.cell_width(),
            cell_height_px: canvas.cell_height(),
        });

        // Canvas, pixels, and view are auto-dropped here
        let flush_res = fb.flush_all();
        if let Err(e) = flush_res {
            beskar_lib::println!("Failed to flush framebuffer: {}", e);
        }
    });
}

const DS_ART: &[&str] = &[
    " ,_~\"\"\"~-, ",
    " .'(_)------`,",
    " |===========|",
    " `,---------,'",
    "   ~-.___.-~  ",
];

#[expect(clippy::cast_possible_truncation)]
const DS_ART_WIDTH: u16 = DS_ART[1].len() as u16;
#[expect(clippy::cast_possible_truncation)]
const DS_ART_HEIGHT: u16 = DS_ART.len() as u16;

fn draw_death_star(canvas: &mut AsciiCanvas<'_>, start_row: u16) -> u16 {
    for (i, line) in DS_ART.iter().enumerate() {
        canvas.write_line_centered(start_row + u16::try_from(i).unwrap(), line);
    }
    DS_ART_HEIGHT
}
