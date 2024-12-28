pub mod hpet;
pub mod tsc;

// TODO: Refactor with a timer-agnostic interface

pub fn wait_ms(ms: u64) {
    if hpet::is_init() {
        hpet::wait_ms(ms);
    } else {
        tsc::wait_ms(ms);
    }
}
