//! Heap data structures for Rust.
//!
//! `rheaps` is an incremental Rust port of
//! [JHeaps](https://github.com/d-michail/jheaps). The public API follows Rust
//! conventions: queries borrow their result and removing from an empty heap
//! returns `None`.

pub mod array;
pub mod dag;
pub mod monotone;
pub mod tree;

/// The common interface implemented by min-oriented heaps.
pub trait Heap<T> {
    /// Inserts `value` into the heap.
    fn push(&mut self, value: T);

    /// Returns a reference to a minimum value, if present.
    fn peek(&self) -> Option<&T>;

    /// Removes and returns a minimum value, if present.
    fn pop(&mut self) -> Option<T>;

    /// Returns the number of values in the heap.
    fn len(&self) -> usize;

    /// Removes all values from the heap.
    fn clear(&mut self);

    /// Returns whether the heap contains no values.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A min-oriented heap that associates each key with a value.
///
/// This trait is separate from [`Heap`] because Rust cannot express the
/// optional value used by Java's `ValueHeap` without requiring a sentinel
/// value. Implementations return the key and value together when removing an
/// entry so neither is lost to ownership.
pub trait ValueHeap<K, V> {
    /// Inserts `key` and its associated `value`.
    fn push(&mut self, key: K, value: V);

    /// Returns the minimum key and its associated value, if present.
    fn peek(&self) -> Option<(&K, &V)>;

    /// Removes and returns the minimum key and its associated value, if
    /// present.
    fn pop(&mut self) -> Option<(K, V)>;

    /// Returns the number of entries in the heap.
    fn len(&self) -> usize;

    /// Removes every entry from the heap.
    fn clear(&mut self);

    /// Returns whether the heap contains no entries.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A heap that supports efficient access to both extrema.
pub trait DoubleEndedHeap<T>: Heap<T> {
    /// Returns a reference to a maximum value, if present.
    fn peek_max(&self) -> Option<&T>;

    /// Removes and returns a maximum value, if present.
    fn pop_max(&mut self) -> Option<T>;
}

/// An addressable heap that supports efficient access to both extrema.
pub trait DoubleEndedAddressableHeap<K, V>: AddressableHeap<K, V> {
    /// Returns the handle, key, and value of a maximum entry, if present.
    fn peek_max(&self) -> Option<(Self::Handle, &K, &V)>;

    /// Removes and returns a maximum entry, if present.
    fn pop_max(&mut self) -> Option<(K, V)>;

    /// Increases the key identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid handle or a key with higher priority
    /// than the current one.
    fn increase_key(&mut self, handle: Self::Handle, key: K)
    -> Result<(), array::IncreaseKeyError>;
}

/// A min-oriented heap whose entries are addressed by stable handles.
///
/// A handle is an opaque capability returned from [`Self::push`]. Its validity
/// is checked by every handle operation; it becomes invalid when the entry is
/// removed or the heap is cleared. Handles cannot be used with another heap.
pub trait AddressableHeap<K, V> {
    /// Opaque type that identifies a live entry in this heap.
    type Handle: Copy + Eq;

    /// Inserts an entry and returns its handle.
    fn push(&mut self, key: K, value: V) -> Self::Handle;

    /// Returns the handle, key, and value of a minimum entry, if present.
    fn peek(&self) -> Option<(Self::Handle, &K, &V)>;

    /// Removes and returns a minimum entry, if present.
    fn pop(&mut self) -> Option<(K, V)>;

    /// Returns the key identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error if `handle` is stale or belongs to another heap.
    fn key(&self, handle: Self::Handle) -> Result<&K, array::InvalidHandle>;

    /// Returns the value identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error if `handle` is stale or belongs to another heap.
    fn value(&self, handle: Self::Handle) -> Result<&V, array::InvalidHandle>;

    /// Replaces the value identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error if `handle` is stale or belongs to another heap.
    fn set_value(&mut self, handle: Self::Handle, value: V) -> Result<(), array::InvalidHandle>;

    /// Decreases the key identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid handle or a key with lower priority
    /// than the current one.
    fn decrease_key(&mut self, handle: Self::Handle, key: K)
    -> Result<(), array::DecreaseKeyError>;

    /// Removes and returns the entry identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error if `handle` is stale or belongs to another heap.
    fn delete(&mut self, handle: Self::Handle) -> Result<(K, V), array::InvalidHandle>;

    /// Returns the number of live entries.
    fn len(&self) -> usize;

    /// Removes all entries and invalidates every outstanding handle.
    fn clear(&mut self);

    /// Returns whether the heap contains no entries.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A heap that can efficiently combine its contents with another heap of the
/// same concrete type.
///
/// A successful meld consumes `other`: it becomes empty and rejects further
/// mutation. Handles created by `other` remain usable through `self` for
/// addressable implementations.
pub trait MeldableHeap<T>: Heap<T> {
    /// Error returned when a meld cannot be performed.
    type MeldError;

    /// Melds `other` into this heap.
    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError>;
}

/// An addressable heap that can efficiently meld another heap.
pub trait MeldableAddressableHeap<K, V>: AddressableHeap<K, V> {
    /// Error returned when a meld cannot be performed.
    type MeldError;

    /// Melds `other` into this heap.
    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError>;
}

/// A double-ended addressable heap that can efficiently meld another heap.
pub trait MeldableDoubleEndedAddressableHeap<K, V>: DoubleEndedAddressableHeap<K, V> {
    /// Error returned when a meld cannot be performed.
    type MeldError;

    /// Melds `other` into this heap.
    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError>;
}
