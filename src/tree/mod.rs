//! Tree-based heap implementations.
//!
//! This module provides leftist, skew, pairing, Fibonacci, rank-pairing, and
//! explicit binary-tree heaps. The meldable heaps retain donor handles after a
//! successful meld; the donor itself is consumed and can no longer accept
//! entries.

mod binary_tree_addressable_heap;
mod core;
mod costless_meld_pairing_heap;
mod fibonacci_heap;
mod leftist_heap;
mod pairing_heap;
mod pure_pairing_heap;
mod rank_pairing_heap;
mod simple_fibonacci_heap;
mod skew_heap;
mod strict_fibonacci_heap;

pub use binary_tree_addressable_heap::BinaryTreeAddressableHeap;
pub use core::{MeldError, TreeHandle};
pub use costless_meld_pairing_heap::CostlessMeldPairingHeap;
pub use fibonacci_heap::FibonacciHeap;
pub use leftist_heap::LeftistHeap;
pub use pairing_heap::PairingHeap;
pub use pure_pairing_heap::PurePairingHeap;
pub use rank_pairing_heap::RankPairingHeap;
pub use simple_fibonacci_heap::SimpleFibonacciHeap;
pub use skew_heap::SkewHeap;
pub use strict_fibonacci_heap::StrictFibonacciHeap;

#[cfg(test)]
mod tests;
