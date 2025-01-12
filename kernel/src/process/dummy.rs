//! Test processes for the kernel.
// TODO: Remove this module

pub fn fibonacci() {
    let mut a = 0_u16;
    let mut b = 1_u16;
    let mut n = 0_u32;

    loop {
        crate::info!(
            "Core {}: Fibonacci({}) = {}",
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
        crate::time::wait_ms(1_000);
    }

    panic!("Thread cannot end for now.")
}

pub fn counter() {
    let mut counter = 0_u64;

    loop {
        crate::info!(
            "Hello, thread 2 on core {}! counter={}",
            crate::locals!().core_id(),
            counter
        );
        counter = counter.wrapping_add(1);
        crate::time::wait_ms(1_000);
    }
}

pub fn idle() {
    loop {
        x86_64::instructions::hlt();
    }
}
