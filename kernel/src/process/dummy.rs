//! Test processes for the kernel.
// TODO: Remove this module

pub extern "C" fn fibonacci() {
    let mut a = 0_u16;
    let mut b = 1_u16;
    let mut n = 0_u32;

    loop {
        crate::info!(
            "Hello from core {}! Fibonacci({}) = {}",
            crate::locals!().core_id(),
            n,
            a
        );
        let Some(next) = a.checked_add(b) else {
            crate::error!("Overflow detected in Fibonacci sequence");
            break;
        };
        a = b;
        b = next;
        n += 1;
        crate::time::wait(crate::time::Duration::from_secs(1));
    }

    loop {
        crate::error!("Fibonacci sequence overflowed");
        crate::time::wait(crate::time::Duration::from_secs(1));
    }
}

pub extern "C" fn counter() {
    let mut counter = 0_u64;

    loop {
        crate::info!(
            "Hello from core {}! counter={}",
            crate::locals!().core_id(),
            counter
        );
        counter = counter.wrapping_add(1);
        crate::time::wait(crate::time::Duration::from_secs(1));
    }
}

pub extern "C" fn floating_point() {
    let mut x = 1.1_f64;

    loop {
        crate::info!(
            "Hello from core {}! x = {:.5}",
            crate::locals!().core_id(),
            x
        );
        x = x * x;
        crate::time::wait(crate::time::Duration::from_secs(1));
    }
}

pub extern "C" fn panic_test() {
    panic!("This is a panic test");
}
