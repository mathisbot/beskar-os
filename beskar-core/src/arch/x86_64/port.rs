use core::marker::PhantomData;

pub mod serial;

trait Sealed {}
#[allow(private_bounds)] // That's the whole point :)
pub trait Access: Sealed {}

pub trait ReadAccess: Access {}
pub trait WriteAccess: Access {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOnly;
impl Sealed for ReadOnly {}
impl Access for ReadOnly {}
impl ReadAccess for ReadOnly {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteOnly;
impl Sealed for WriteOnly {}
impl Access for WriteOnly {}
impl WriteAccess for WriteOnly {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadWrite;
impl Sealed for ReadWrite {}
impl Access for ReadWrite {}
impl ReadAccess for ReadWrite {}
impl WriteAccess for ReadWrite {}

#[allow(private_bounds)] // That's the whole point :)
/// A marker trait for types which can be accessed through a port.
pub trait PortAccessible: Sealed {
    #[must_use]
    unsafe fn read_from_port(port: u16) -> Self;
    unsafe fn write_to_port(port: u16, value: Self);
}

impl Sealed for u8 {}
impl PortAccessible for u8 {
    #[must_use]
    #[inline]
    unsafe fn read_from_port(port: u16) -> Self {
        let value: u8;
        unsafe {
            core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
        }
        value
    }

    #[inline]
    unsafe fn write_to_port(port: u16, value: Self) {
        unsafe {
            core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
        }
    }
}
impl Sealed for u16 {}
impl PortAccessible for u16 {
    #[must_use]
    #[inline]
    unsafe fn read_from_port(port: u16) -> Self {
        let value: u16;
        unsafe {
            core::arch::asm!("in ax, dx", out("ax") value, in("dx") port, options(nomem, nostack, preserves_flags));
        }
        value
    }

    #[inline]
    unsafe fn write_to_port(port: u16, value: Self) {
        unsafe {
            core::arch::asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags));
        }
    }
}
impl Sealed for u32 {}
impl PortAccessible for u32 {
    #[must_use]
    #[inline]
    unsafe fn read_from_port(port: u16) -> Self {
        let value: u32;
        unsafe {
            core::arch::asm!("in eax, dx", out("eax") value, in("dx") port, options(nomem, nostack, preserves_flags));
        }
        value
    }

    #[inline]
    unsafe fn write_to_port(port: u16, value: Self) {
        unsafe {
            core::arch::asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port<T: PortAccessible, A: Access> {
    port: u16,
    phantom: PhantomData<(T, A)>,
}

impl<T: PortAccessible, A: Access> Port<T, A> {
    #[must_use]
    #[inline]
    pub const fn new(port: u16) -> Self {
        Self {
            port,
            phantom: PhantomData,
        }
    }
}

impl<T: PortAccessible, A: ReadAccess> Port<T, A> {
    #[must_use]
    #[inline]
    pub unsafe fn read(&self) -> T {
        unsafe { T::read_from_port(self.port) }
    }
}

impl<T: PortAccessible, A: WriteAccess> Port<T, A> {
    #[inline]
    pub unsafe fn write(&self, value: T) {
        unsafe { T::write_to_port(self.port, value) }
    }
}
