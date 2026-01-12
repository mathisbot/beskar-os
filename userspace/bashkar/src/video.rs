pub mod screen;
pub mod tty;
pub mod ui;

#[inline]
pub fn init() {
    screen::init();
    ui::draw();
    tty::init();
}
