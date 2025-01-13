//! Test processes for the kernel.
// TODO: Remove this module

pub fn fibonacci() {
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
        crate::time::wait_ms(1_000);
    }

    loop {
        crate::error!("Fibonacci sequence overflowed");
        crate::time::wait_ms(1_000);
    }
}

pub fn counter() {
    let mut counter = 0_u64;

    loop {
        crate::info!(
            "Hello from core {}! counter={}",
            crate::locals!().core_id(),
            counter
        );
        counter = counter.wrapping_add(1);
        crate::time::wait_ms(1_000);
    }
}

pub fn hello_world() {
    loop {
        crate::info!("Hello from core {}!", crate::locals!().core_id());
        crate::time::wait_ms(1_000);
    }
}

pub fn alloc_intensive() {
    loop {
        let sz = usize::from(unsafe { crate::cpu::rand::rand::<u16>() }) * 32;
        let vec = alloc::vec![0_u8; sz];
        crate::info!(
            "Hello from core {}! Allocated {} bytes",
            crate::locals!().core_id(),
            vec.len()
        );
        crate::time::wait_ms(1_000);
    }
}

pub fn panic_test() {
    panic!("This is a panic test");
}

pub fn idle() {
    loop {
        x86_64::instructions::hlt();
    }
}
