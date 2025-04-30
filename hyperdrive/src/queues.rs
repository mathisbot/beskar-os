//! Queues module.
//!
//! Queues are FILO data structures.
//!
//! ## Modules
//!
//! - `mpsc` : Multiple-producer single-consumer queue.
//! - `ring` : Ring queue backed by a fixed-size array.

pub mod mpsc;
pub mod ring;
