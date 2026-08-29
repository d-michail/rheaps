//! Tree-based heap implementations.
//!
//! This first batch provides leftist, skew, pairing, and explicit binary-tree
//! heaps. The meldable heaps retain donor handles after a successful meld;
//! the donor itself is consumed and can no longer accept entries.

mod binary_tree_addressable_heap;
mod core;
mod leftist_heap;
mod pairing_heap;
mod skew_heap;

pub use binary_tree_addressable_heap::BinaryTreeAddressableHeap;
pub use core::{MeldError, TreeHandle};
pub use leftist_heap::LeftistHeap;
pub use pairing_heap::PairingHeap;
pub use skew_heap::SkewHeap;

#[cfg(test)]
mod tests;
