# Hyperdrive

This package contains numerous utility structures for use in `no_std` environments.
These are intended for use by BeskarOS components, in particular the kernel.

## Structures

It defines:
- Pointers
    - Volatile Pointers with compile-time Access Rights
- Locks
    - Mellor, Crumley and Scott
    - Read-Write
- Flow control
    - Once
- Queues
    - Multiple Producer, Single Consumer and intrusive
- Sync
    - Barrier
