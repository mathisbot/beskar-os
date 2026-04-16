use beskar_core::video::Pixel;

use crate::{core::TestResult, ensure, ensure_eq};

pub fn test_surface_api() -> TestResult {
    screen_info()?;
    surface()?;

    Ok(())
}

fn screen_info() -> TestResult {
    let Ok(surface) = beskar_lib::surface::query_screen_info() else {
        return Err("failed to query screen info");
    };

    ensure!(
        surface.width() != 0 && surface.height() != 0,
        "invalid screen dimensions"
    );

    Ok(())
}

fn surface() -> TestResult {
    let mut buffer = [Pixel::WHITE; 1];

    let Ok(surface) =
        (unsafe { beskar_lib::surface::Surface::create(1, 1, 0, 0, buffer.as_mut_ptr()) })
    else {
        return Err("failed to create surface");
    };

    ensure_eq!(surface.width(), 1, "surface width mismatch");
    ensure_eq!(surface.height(), 1, "surface height mismatch");

    ensure!(
        surface.mark_all_dirty().is_ok(),
        "failed to mark surface dirty"
    );
    ensure!(surface.present_dirty().is_ok(), "failed to present surface");

    Ok(())
}
