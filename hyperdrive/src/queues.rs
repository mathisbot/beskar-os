//! Queues module.
//!
//! Queues are FILO data structures.
//!
//! ## Modules
//!
//! - `mpmc` : Multiple-producer multiple-consumer queue.
//! - `mpsc` : Multiple-producer single-consumer queue.
//! - `ring` : Ring queue backed by a fixed-size array.

pub mod mpmc;
pub mod mpsc;
pub mod ring;
