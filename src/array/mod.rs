//! Array-backed heap implementations.

mod addressable_heap;
mod binary_array_heap;
mod binary_array_integer_value_heap;
mod binary_array_weak_heap;
mod dary_array_heap;
mod min_max_binary_array_double_ended_heap;

pub use addressable_heap::{
    AddressableHandle, BinaryArrayAddressableHeap, DaryArrayAddressableHeap, DecreaseKeyError,
    IncreaseKeyError, InvalidHandle,
};
pub use binary_array_heap::BinaryArrayHeap;
pub use binary_array_integer_value_heap::BinaryArrayIntegerValueHeap;
pub use binary_array_weak_heap::{BinaryArrayBulkInsertWeakHeap, BinaryArrayWeakHeap};
pub use dary_array_heap::{DaryArrayHeap, InvalidDegree};
pub use min_max_binary_array_double_ended_heap::MinMaxBinaryArrayDoubleEndedHeap;

#[cfg(test)]
mod tests;
