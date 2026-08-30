use crate::{Heap, MeldableHeap};

use super::soft_heap_core::{SoftHeapCore, SoftHeapError, SoftMeldError};

/// A Kaplan-Zwick binary-tree soft heap.
///
/// A soft heap may return keys out of order: after inserting `n` keys, at
/// most `error_rate * n` live keys may have had their priority corrupted.
/// The original keys are always returned, exactly once. Keys must be cloneable
/// because a soft heap retains a corrupted-key snapshot while returning the
/// original key by value.
pub struct BinaryTreeSoftHeap<K> {
    core: SoftHeapCore<K, ()>,
}

impl<K: Ord + Clone> BinaryTreeSoftHeap<K> {
    /// Creates a heap with an error rate strictly between zero and one.
    pub fn new(error_rate: f64) -> Result<Self, SoftHeapError> {
        Ok(Self {
            core: SoftHeapCore::new(error_rate)?,
        })
    }
}

impl<K: Ord + Clone> BinaryTreeSoftHeap<K> {
    /// Returns the rank below which keys are never corrupted.
    #[must_use]
    pub const fn rank_limit(&self) -> usize {
        self.core.rank_limit()
    }

    /// Inserts a key.
    pub fn push(&mut self, key: K) {
        self.core.insert(key, ());
    }

    /// Returns the next key selected by the soft heap.
    #[must_use]
    pub fn peek(&self) -> Option<&K> {
        self.core.peek_entry().map(|(_, key, _)| key)
    }

    /// Removes and returns the next key selected by the soft heap.
    pub fn pop(&mut self) -> Option<K> {
        self.core.pop_item().map(|item| item.into_pair().0)
    }

    /// Returns the number of live keys.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.core.len()
    }

    /// Returns whether the heap contains no keys.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.core.len() == 0
    }

    /// Removes all keys.
    pub fn clear(&mut self) {
        self.core.clear();
    }
}

impl<K: Ord + Clone> BinaryTreeSoftHeap<K> {
    /// Melds `other` into this heap, consuming the donor on success.
    pub fn meld(&mut self, other: Self) -> Result<(), SoftMeldError> {
        if self.rank_limit() != other.rank_limit() {
            return Err(SoftMeldError::IncompatibleErrorRate);
        }
        self.core.meld_from(other.core);
        Ok(())
    }
}

impl<T: Ord + Clone> Heap<T> for BinaryTreeSoftHeap<T> {
    fn push(&mut self, value: T) {
        Self::push(self, value);
    }

    fn peek(&self) -> Option<&T> {
        Self::peek(self)
    }

    fn pop(&mut self) -> Option<T> {
        Self::pop(self)
    }

    fn len(&self) -> usize {
        Self::len(self)
    }

    fn clear(&mut self) {
        Self::clear(self);
    }
}

impl<T: Ord + Clone> MeldableHeap<T> for BinaryTreeSoftHeap<T> {
    type MeldError = SoftMeldError;

    fn meld(&mut self, other: Self) -> Result<(), Self::MeldError> {
        Self::meld(self, other)
    }
}
