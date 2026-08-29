use crate::array::{DecreaseKeyError, InvalidHandle};
use crate::{AddressableHeap, MeldableAddressableHeap};

use super::soft_heap_core::{SoftHandle, SoftHeapCore, SoftHeapError, SoftMeldError};

/// An addressable Kaplan-Zwick binary-tree soft heap.
///
/// Keys may be returned out of order according to the configured error rate.
/// As in JHeaps, key decreases are not supported; values, deletion, and melds
/// retain checked opaque handles.
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

    /// Inserts an entry unless this heap was consumed as a meld donor.
    pub fn try_insert(&mut self, key: K, value: V) -> Result<SoftHandle, SoftMeldError> {
        self.core.insert(key, value)
    }

    /// Inserts an entry and returns a checked handle.
    pub fn insert(&mut self, key: K, value: V) -> SoftHandle {
        self.try_insert(key, value)
            .expect("a meld donor cannot accept new entries")
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

    /// Replaces the value identified by `handle`.
    pub fn set_value(&mut self, handle: SoftHandle, value: V) -> Result<(), InvalidHandle> {
        self.core.set_value(handle, value)
    }

    /// Reports that binary-tree soft heaps do not support key decreases.
    pub fn decrease_key(&mut self, handle: SoftHandle, _key: K) -> Result<(), DecreaseKeyError> {
        self.core
            .validate_handle(handle)
            .map_err(DecreaseKeyError::InvalidHandle)?;
        Err(DecreaseKeyError::Unsupported)
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
    pub fn meld(&mut self, other: &mut Self) -> Result<(), SoftMeldError> {
        if !self.core.active() {
            return Err(SoftMeldError::ReceiverConsumed);
        }
        if !other.core.active() {
            return Err(SoftMeldError::DonorConsumed);
        }
        if self.rank_limit() != other.rank_limit() {
            return Err(SoftMeldError::IncompatibleErrorRate);
        }
        self.core.meld_from(&mut other.core);
        Ok(())
    }
}

impl<K: Ord + Clone, V> AddressableHeap<K, V> for BinaryTreeSoftAddressableHeap<K, V> {
    type Handle = SoftHandle;

    fn push(&mut self, key: K, value: V) -> Self::Handle {
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

    fn set_value(&mut self, handle: Self::Handle, value: V) -> Result<(), InvalidHandle> {
        Self::set_value(self, handle, value)
    }

    fn decrease_key(&mut self, handle: Self::Handle, key: K) -> Result<(), DecreaseKeyError> {
        Self::decrease_key(self, handle, key)
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

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        Self::meld(self, other)
    }
}
