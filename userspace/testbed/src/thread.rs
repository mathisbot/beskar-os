use crate::{core::TestResult, ensure};
use beskar_core::time::Duration;
use beskar_lib::sync::{Condvar, Mutex};
use core::sync::atomic::{AtomicBool, Ordering};

pub fn test_thread_api() -> TestResult {
    thread_spawn()?;
    thread_condvar()?;

    Ok(())
}

fn thread_spawn() -> TestResult {
    const TIMEOUT: Duration = Duration::from_secs(1);

    let Ok(tid) = beskar_lib::thread::spawn(setting_thread) else {
        return Err("thread spawn failed");
    };
    ensure!(tid != 0, "Thread spawned with invalid TID 0");

    let start = beskar_lib::time::now();
    while !THREAD_RAN.load(Ordering::SeqCst) && beskar_lib::time::elapsed(start) < TIMEOUT {
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

fn thread_condvar() -> TestResult {
    const TIMEOUT: Duration = Duration::from_secs(1);
    const TMOUT_TO_REACH: Duration = Duration::from_micros(1);

    {
        let guard = CV_MUTEX.lock();

        let Ok(tid) = beskar_lib::thread::spawn(condvar_thread) else {
            return Err("thread spawn failed");
        };
        ensure!(tid != 0, "Thread spawned with invalid TID 0");

        let (_, reason) = CONDVAR.wait_for(guard, TIMEOUT);
        ensure!(
            !reason.timed_out(),
            "Thread did not signal condvar within expected time"
        );
    }

    {
        let guard = CV_MUTEX.lock();
        let (_, reason) = CONDVAR.wait_for(guard, TMOUT_TO_REACH);
        ensure!(
            reason.timed_out(),
            "Thread signaled condvar when it should not have",
        );
    }

    Ok(())
}

static CONDVAR: Condvar = Condvar::new();
static CV_MUTEX: Mutex<()> = Mutex::new(());

extern "C" fn condvar_thread() -> ! {
    {
        let _guard = CV_MUTEX.lock();
        let n = CONDVAR.notify_all();
        debug_assert_eq!(n, 1);
    }
    beskar_lib::exit(beskar_lib::ExitCode::Success);
}
