use crate::{core::TestResult, ensure};
use beskar_core::time::Duration;
use core::sync::atomic::{AtomicBool, Ordering};

pub fn test_thread_api() -> TestResult {
    thread_spawn()?;

    Ok(())
}

fn thread_spawn() -> TestResult {
    const TIMEOUT: Duration = Duration::from_secs(3);

    let Ok(tid) = beskar_lib::thread::spawn(setting_thread) else {
        return Err("thread spawn failed");
    };
    ensure!(tid != 0, "Thread spawned with invalid TID 0");

    let start = beskar_lib::time::now();
    let end = start + TIMEOUT;
    while !THREAD_RAN.load(Ordering::SeqCst) && beskar_lib::time::now() < end {
        // TODO: Yield
        core::hint::spin_loop();
    }

    ensure!(
        THREAD_RAN.load(Ordering::SeqCst),
        "thread did not run within expected time",
    );

    Ok(())
}

static THREAD_RAN: AtomicBool = AtomicBool::new(false);

extern "C" fn setting_thread() -> ! {
    THREAD_RAN.store(true, Ordering::SeqCst);
    beskar_lib::exit(beskar_lib::ExitCode::Success);
}
