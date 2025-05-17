use screen::with_screen;

pub mod assets;
pub mod screen;
pub mod tty;

#[inline]
pub fn init() {
    screen::init();

    draw_logo_image();

    tty::init();
}

fn draw_logo_image() {
    let raw_img = assets::banner::BANNER_RAW;

    let screen_info = screen::screen_info();
    let screen_width = usize::from(screen_info.width());
    let screen_height = usize::from(screen_info.height());
    let screen_stride = usize::from(screen_info.stride());
    let bpp = usize::from(screen_info.bytes_per_pixel());

    assert!(
        screen_width >= assets::banner::BANNER_WIDTH
            && screen_height >= assets::banner::BANNER_HEIGHT,
        "Image doesn't fit on screen"
    );

    with_screen(|screen| {
        let buffer_mut = screen.buffer_mut();

        for (row_nb, row) in raw_img
            .chunks_exact(bpp * assets::banner::BANNER_WIDTH)
            .enumerate()
        {
            let row_start = row_nb * screen_stride * bpp;
            let row_end = row_start + bpp * assets::banner::BANNER_WIDTH;

            buffer_mut[row_start..row_end].copy_from_slice(row);
        }

        screen.flush(None);
        // screen.flush(Some(0..assets::banner::BANNER_HEIGHT.try_into().unwrap()));
    });
}
