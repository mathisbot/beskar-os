pub mod screen;
pub mod tty;
pub mod ui;

#[inline]
pub fn init() {
    screen::init_ui_surface();
    ui::draw();
    tty::init();
}
