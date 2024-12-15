// TODO: Implement the scheduler

use crate::utils::once::Once;

pub mod thread;

static SCHEDULER: Once<Scheduler> = Once::uninit();

pub fn init(_current_thread: thread::Thread) {
    SCHEDULER.call_once(|| Scheduler {});
}

pub struct Scheduler {}
