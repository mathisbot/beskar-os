# Heaperion

A high-performance, `no_std` heap allocator written in Rust.

## Slab Allocator

Optimized for small allocations, O(1) allocation and deallocation.

## Buddy Allocator

Optimized for larger allocations, O(log(c)) allocation and deallocation.

## Hybrid Allocator

Dispatch allocations to Slab or Buddy depending on the size.

## GrowableHeap

Wrapper around heaps that can grow at runtime.
