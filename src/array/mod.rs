//! Array-backed heap implementations.
//!
//! These heaps store entries in a contiguous backing array rather than a
//! linked node structure, which makes them the smallest and most
//! cache-friendly heaps in the crate. They are the right default unless a
//! workload specifically needs efficient meld, monotone keys, or soft-heap
//! corruption.
//!
//! - [`BinaryArrayHeap`] and [`DaryArrayHeap`] are ordinary value-only heaps;
//!   [`BinaryArrayAddressableHeap`] and [`DaryArrayAddressableHeap`] add
//!   handle-based `decrease_key` and `delete`.
//! - [`BinaryArrayWeakHeap`] and [`BinaryArrayBulkInsertWeakHeap`] trade a
//!   slightly relaxed heap invariant for fewer comparisons per operation.
//! - [`BinaryArrayIntegerValueHeap`] specializes the key type to `i32` for
//!   lower per-entry overhead than a generic key.
//! - [`MinMaxBinaryArrayDoubleEndedHeap`] exposes both the minimum and
//!   maximum in a single array-backed heap.

mod addressable_heap;
mod binary_array_heap;
mod binary_array_integer_value_heap;
mod binary_array_weak_heap;
mod dary_array_heap;
mod min_max_binary_array_double_ended_heap;

pub use addressable_heap::{
    AddressableHandle, BinaryArrayAddressableHeap, DaryArrayAddressableHeap,
};
pub use binary_array_heap::BinaryArrayHeap;
pub use binary_array_integer_value_heap::{
    BinaryArrayIntegerValueHeap, Iter as IntegerValueHeapIter,
};
pub use binary_array_weak_heap::{BinaryArrayBulkInsertWeakHeap, BinaryArrayWeakHeap};
pub use dary_array_heap::{DaryArrayHeap, InvalidDegree};
pub use min_max_binary_array_double_ended_heap::MinMaxBinaryArrayDoubleEndedHeap;

#[cfg(test)]
mod tests;
