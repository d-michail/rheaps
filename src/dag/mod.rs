//! Directed-acyclic-graph heap implementations.
//!
//! This module currently provides a hollow heap. Hollow heaps use a DAG of
//! full and hollow nodes so decreasing a non-minimum key does not require
//! cutting the old node from its parent.

mod hollow_heap;

pub use hollow_heap::{HollowHandle, HollowHeap};

#[cfg(test)]
mod tests;
