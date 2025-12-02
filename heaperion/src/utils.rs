/// Align `addr` upwards to `align`.
///
/// Requires that `align` is a power of two.
#[inline]
pub const fn align_up(addr: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (addr + align - 1) & !(align - 1)
}

/// Calculate the order (log2) of a size for buddy allocator
#[inline]
pub const fn size_to_order(size: usize) -> usize {
    if size <= 1 {
        return 0;
    }
    let next_power = size.next_power_of_two();
    (usize::BITS - next_power.leading_zeros() - 1) as usize
}

/// Calculate size from order
#[inline]
pub const fn order_to_size(order: usize) -> usize {
    1 << order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(1, 8), 8);
        assert_eq!(align_up(7, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 8), 16);
        assert_eq!(align_up(100, 16), 112);
    }

    #[test]
    fn test_size_to_order() {
        assert_eq!(size_to_order(1), 0);
        assert_eq!(size_to_order(2), 1);
        assert_eq!(size_to_order(3), 2);
        assert_eq!(size_to_order(4), 2);
        assert_eq!(size_to_order(8), 3);
        assert_eq!(size_to_order(16), 4);
    }

    #[test]
    fn test_order_to_size() {
        assert_eq!(order_to_size(0), 1);
        assert_eq!(order_to_size(1), 2);
        assert_eq!(order_to_size(2), 4);
        assert_eq!(order_to_size(3), 8);
        assert_eq!(order_to_size(10), 1024);
    }
}
