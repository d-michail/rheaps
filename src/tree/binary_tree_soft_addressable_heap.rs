use crate::error::InvalidHandle;
use crate::{AddressableHeap, Heap, MeldableAddressableHeap, MeldableHeap};

use super::soft_heap_core::{SoftHandle, SoftHeapCore, SoftHeapError, SoftMeldError};

/// An addressable Kaplan-Zwick binary-tree soft heap.
///
/// Keys may be returned out of order according to the configured error rate.
/// As in JHeaps, key decreases are not supported: this heap does not
/// implement [`crate::DecreaseKeyHeap`] because its corruption-bounded
/// structure does not track precise per-entry positions. Values, deletion,
/// and melds retain checked opaque handles.
///
/// ```
/// use rheaps::tree::BinaryTreeSoftAddressableHeap;
///
/// let mut heap = BinaryTreeSoftAddressableHeap::new(0.1).unwrap();
/// let handle = heap.insert(4, "clean up");
/// heap.insert(1, "reply to mail");
///
/// assert_eq!(heap.len(), 2);
/// assert_eq!(heap.value(handle), Ok(&"clean up"));
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BinaryTreeSoftAddressableHeap<K, V = ()> {
    core: SoftHeapCore<K, V>,
}

impl<K: Ord + Clone, V> BinaryTreeSoftAddressableHeap<K, V> {
    /// Creates a heap with an error rate strictly between zero and one.
    pub fn new(error_rate: f64) -> Result<Self, SoftHeapError> {
        Ok(Self {
            core: SoftHeapCore::new(error_rate)?,
        })
    }
}

impl<K: Ord + Clone, V> BinaryTreeSoftAddressableHeap<K, V> {
    /// Returns the rank below which keys are never corrupted.
    #[must_use]
    pub const fn rank_limit(&self) -> usize {
        self.core.rank_limit()
    }

    /// Inserts an entry and returns a checked handle.
    pub fn insert(&mut self, key: K, value: V) -> SoftHandle {
        self.core.insert(key, value)
    }

    /// Returns the handle, key, and value selected by the soft heap.
    #[must_use]
    pub fn peek_entry(&self) -> Option<(SoftHandle, &K, &V)> {
        self.core.peek_entry()
    }

    /// Removes and returns the entry selected by the soft heap.
    pub fn pop_entry(&mut self) -> Option<(K, V)> {
        self.core.pop_item().map(|item| item.into_pair())
    }

    /// Returns the key identified by `handle`.
    pub fn key(&self, handle: SoftHandle) -> Result<&K, InvalidHandle> {
        self.core.key(handle)
    }

    /// Returns the value identified by `handle`.
    pub fn value(&self, handle: SoftHandle) -> Result<&V, InvalidHandle> {
        self.core.value(handle)
    }

    /// Returns mutable access to the value identified by `handle`.
    pub fn value_mut(&mut self, handle: SoftHandle) -> Result<&mut V, InvalidHandle> {
        self.core.value_mut(handle)
    }

    /// Removes and returns the entry identified by `handle`.
    pub fn delete(&mut self, handle: SoftHandle) -> Result<(K, V), InvalidHandle> {
        self.core.delete(handle).map(|item| item.into_pair())
    }

    /// Returns the number of live entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.core.len()
    }

    /// Returns whether this heap contains no entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.core.len() == 0
    }

    /// Removes all entries and invalidates every outstanding handle.
    pub fn clear(&mut self) {
        self.core.clear();
    }
}

impl<K: Ord + Clone, V> BinaryTreeSoftAddressableHeap<K, V> {
    /// Melds `other` into this heap, consuming the donor on success.
    pub fn meld(&mut self, other: Self) -> Result<(), SoftMeldError> {
        if self.rank_limit() != other.rank_limit() {
            return Err(SoftMeldError::IncompatibleErrorRate);
        }
        self.core.meld_from(other.core);
        Ok(())
    }
}

impl<K: Ord + Clone, V> AddressableHeap<K, V> for BinaryTreeSoftAddressableHeap<K, V> {
    type Handle = SoftHandle;

    fn insert(&mut self, key: K, value: V) -> Self::Handle {
        Self::insert(self, key, value)
    }

    fn peek(&self) -> Option<(Self::Handle, &K, &V)> {
        Self::peek_entry(self)
    }

    fn pop(&mut self) -> Option<(K, V)> {
        Self::pop_entry(self)
    }

    fn key(&self, handle: Self::Handle) -> Result<&K, InvalidHandle> {
        Self::key(self, handle)
    }

    fn value(&self, handle: Self::Handle) -> Result<&V, InvalidHandle> {
        Self::value(self, handle)
    }

    fn value_mut(&mut self, handle: Self::Handle) -> Result<&mut V, InvalidHandle> {
        Self::value_mut(self, handle)
    }

    fn delete(&mut self, handle: Self::Handle) -> Result<(K, V), InvalidHandle> {
        Self::delete(self, handle)
    }

    fn len(&self) -> usize {
        Self::len(self)
    }

    fn clear(&mut self) {
        Self::clear(self);
    }
}

impl<K: Ord + Clone, V> MeldableAddressableHeap<K, V> for BinaryTreeSoftAddressableHeap<K, V> {
    type MeldError = SoftMeldError;

    fn meld(&mut self, other: Self) -> Result<(), Self::MeldError> {
        Self::meld(self, other)
    }
}

impl<K: Ord + Clone> BinaryTreeSoftAddressableHeap<K, ()> {
    /// Inserts a key into this value-less heap and returns a checked handle.
    pub fn push(&mut self, key: K) -> SoftHandle {
        self.insert(key, ())
    }

    /// Returns the next key selected by the soft heap, if present.
    #[must_use]
    pub fn peek(&self) -> Option<&K> {
        self.peek_entry().map(|(_, key, _)| key)
    }

    /// Removes and returns the next key selected by the soft heap.
    pub fn pop(&mut self) -> Option<K> {
        self.pop_entry().map(|(key, ())| key)
    }
}

impl<T: Ord + Clone> Heap<T> for BinaryTreeSoftAddressableHeap<T, ()> {
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

impl<T: Ord + Clone> MeldableHeap<T> for BinaryTreeSoftAddressableHeap<T, ()> {
    type MeldError = SoftMeldError;

    fn meld(&mut self, other: Self) -> Result<(), Self::MeldError> {
        Self::meld(self, other)
    }
}
