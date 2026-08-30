//! Tree-based heap implementations.
//!
//! This module provides explicit binary and d-ary addressable heaps,
//! Kaplan-Zwick soft heaps, pairing and Fibonacci heap families, and reflected
//! double-ended heaps. The meldable heaps take their donor by value, so it is
//! moved into the meld call; donor handles remain valid through the receiver.

mod binary_tree_addressable_heap;
mod binary_tree_soft_addressable_heap;
mod binary_tree_soft_heap;
mod core;
mod costless_meld_pairing_heap;
mod dary_tree_addressable_heap;
mod fibonacci_heap;
mod leftist_heap;
mod pairing_heap;
mod pure_pairing_heap;
mod rank_pairing_heap;
mod reflected_heap;
mod simple_fibonacci_heap;
mod skew_heap;
mod soft_heap_core;
mod strict_fibonacci_heap;

pub use binary_tree_addressable_heap::BinaryTreeAddressableHeap;
pub use binary_tree_soft_addressable_heap::BinaryTreeSoftAddressableHeap;
pub use binary_tree_soft_heap::BinaryTreeSoftHeap;
pub use core::TreeHandle;
pub use costless_meld_pairing_heap::CostlessMeldPairingHeap;
pub use dary_tree_addressable_heap::{DaryTreeAddressableHeap, InvalidBranchingFactor};
pub use fibonacci_heap::FibonacciHeap;
pub use leftist_heap::LeftistHeap;
pub use pairing_heap::PairingHeap;
pub use pure_pairing_heap::PurePairingHeap;
pub use rank_pairing_heap::RankPairingHeap;
pub use reflected_heap::{ReflectedFibonacciHeap, ReflectedHandle, ReflectedPairingHeap};
pub use simple_fibonacci_heap::SimpleFibonacciHeap;
pub use skew_heap::SkewHeap;
pub use soft_heap_core::{SoftHandle, SoftHeapError, SoftMeldError};
pub use strict_fibonacci_heap::StrictFibonacciHeap;

#[cfg(test)]
mod tests;
