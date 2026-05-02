use crate::{core::TestResult, ensure_eq};

pub fn test_sync_api() -> TestResult {
    mutex()?;
    // TODO: Test actual synchronization ;)

    Ok(())
}

fn mutex() -> TestResult {
    let mutex = beskar_lib::sync::Mutex::new(0);
    {
        let mut guard = mutex.lock();
        ensure_eq!(*guard, 0, "Mutex did not initialize to expected value");
        *guard += 1;
    }
    {
        let guard = mutex.lock();
        ensure_eq!(*guard, 1, "Mutex did not preserve state across locks");
    }
    Ok(())
}
