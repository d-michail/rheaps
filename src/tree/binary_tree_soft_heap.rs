use crate::array::{Comparator, NaturalOrder};
use crate::{Heap, MeldableHeap};

use super::soft_heap_core::{SoftHeapCore, SoftHeapError, SoftMeldError};

/// A Kaplan-Zwick binary-tree soft heap.
///
/// A soft heap may return keys out of order: after inserting `n` keys, at
/// most `error_rate * n` live keys may have had their priority corrupted.
/// The original keys are always returned, exactly once. Keys must be cloneable
/// because a soft heap retains a corrupted-key snapshot while returning the
/// original key by value.
pub struct BinaryTreeSoftHeap<K, C = NaturalOrder> {
    core: SoftHeapCore<K, (), C>,
}

impl<K: Ord + Clone> BinaryTreeSoftHeap<K> {
    /// Creates a heap with an error rate strictly between zero and one.
    pub fn new(error_rate: f64) -> Result<Self, SoftHeapError> {
        Self::with_comparator(error_rate, NaturalOrder)
    }
}

impl<K, C> BinaryTreeSoftHeap<K, C>
where
    K: Clone,
    C: Comparator<K>,
{
    /// Creates a heap with an error rate strictly between zero and one,
    /// ordered by `compare`.
    pub fn with_comparator(error_rate: f64, compare: C) -> Result<Self, SoftHeapError> {
        Ok(Self {
            core: SoftHeapCore::new(error_rate, compare)?,
        })
    }

    /// Returns the comparator used to order corrupted keys.
    #[must_use]
    pub fn comparator(&self) -> &C {
        self.core.comparator()
    }

    /// Returns the rank below which keys are never corrupted.
    #[must_use]
    pub const fn rank_limit(&self) -> usize {
        self.core.rank_limit()
    }

    /// Inserts a key unless this heap was consumed as a meld donor.
    pub fn try_insert(&mut self, key: K) -> Result<(), SoftMeldError> {
        self.core.insert(key, ()).map(|_| ())
    }

    /// Inserts a key.
    pub fn insert(&mut self, key: K) {
        self.try_insert(key)
            .expect("a meld donor cannot accept new entries");
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

impl<K, C> BinaryTreeSoftHeap<K, C>
where
    K: Clone,
    C: Comparator<K> + PartialEq,
{
    /// Melds `other` into this heap, consuming the donor on success.
    pub fn meld(&mut self, other: &mut Self) -> Result<(), SoftMeldError> {
        if !self.core.active() {
            return Err(SoftMeldError::ReceiverConsumed);
        }
        if !other.core.active() {
            return Err(SoftMeldError::DonorConsumed);
        }
        if self.comparator() != other.comparator() {
            return Err(SoftMeldError::IncompatibleComparator);
        }
        if self.rank_limit() != other.rank_limit() {
            return Err(SoftMeldError::IncompatibleErrorRate);
        }
        self.core.meld_from(&mut other.core);
        Ok(())
    }
}

impl<T, C> Heap<T> for BinaryTreeSoftHeap<T, C>
where
    T: Clone,
    C: Comparator<T>,
{
    fn push(&mut self, value: T) {
        self.insert(value);
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

impl<T, C> MeldableHeap<T> for BinaryTreeSoftHeap<T, C>
where
    T: Clone,
    C: Comparator<T> + PartialEq,
{
    type MeldError = SoftMeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        Self::meld(self, other)
    }
}
