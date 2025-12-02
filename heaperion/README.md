# Heaperion

A high-performance, `no_std` heap allocator written in Rust.

## Overview

Heaperion is a robust and efficient memory allocator designed for embedded systems and bare-metal environments. It combines multiple allocation strategies to provide optimal performance across different allocation sizes and patterns.

## Features

- `no_std` compatible: Works in embedded and bare-metal environments
- Hybrid allocation strategy: Combines slab and buddy allocators for optimal performance
- Zero runtime overhead: All metadata stored inline with allocations
- Minimal fragmentation: Intelligent allocation strategies minimize waste

## Architecture

### Slab Allocator

- Optimized for small allocations
- O(1) allocation and deallocation
- Pre-sized pools for common allocation sizes
- Excellent cache locality

### Buddy Allocator

- Optimized for larger allocations
- O(log(c)) allocation and deallocation
- Power-of-two sized blocks
- Automatic coalescing to reduce fragmentation
