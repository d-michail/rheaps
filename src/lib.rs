//! Heap and priority-queue data structures for Rust.
//!
//! `rheaps` is an idiomatic Rust port of
//! [JHeaps](https://github.com/d-michail/jheaps), a mature Java heap library.
//! It packages array, tree, DAG, double-ended, addressable, meldable, soft,
//! and monotone heaps behind a small set of common traits, so generic code
//! can be written against a capability (say, [`AddressableHeap`]) instead of
//! a concrete type.
//!
//! # Quick start
//!
//! ```
//! use rheaps::Heap;
//! use rheaps::array::BinaryArrayHeap;
//!
//! let mut heap = BinaryArrayHeap::new();
//! heap.push(4);
//! heap.push(1);
//! heap.push(3);
//!
//! assert_eq!(heap.peek(), Some(&1));
//! assert_eq!(heap.pop(), Some(1));
//! ```
//!
//! # Choosing an implementation
//!
//! | Module        | Representative types                                                    | Reach for it when you need                                              |
//! |---------------|---------------------------------------------------------------------------|---------------------------------------------------------------------------|
//! | [`mod@array`] | `BinaryArrayHeap`, `DaryArrayHeap`, weak heaps                            | the smallest, cache-friendly heap for `push`/`pop`, optionally addressable |
//! | [`tree`]      | leftist, skew, pairing, rank-pairing, Fibonacci, soft, and reflected heaps | efficient meld, amortized O(1) decrease-key, or both minimum and maximum access |
//! | [`dag`]       | `HollowHeap`                                                              | meld and decrease-key without cutting nodes from a parent                 |
//! | [`monotone`]  | radix heaps over `u32`, `u64`, `FiniteF64`, and `BigUint`                  | keys are removed in nondecreasing order, e.g. Dijkstra's algorithm         |
//!
//! Each module's own documentation lists its concrete types and the common
//! traits each one implements.
//!
//! # Concepts
//!
//! - **Ordering.** Keys use their [`Ord`] implementation, and duplicate keys
//!   are permitted. Wrap a key in a newtype (or [`std::cmp::Reverse`]) to
//!   change its priority order; for example, `Reverse` turns any
//!   min-oriented heap into a max-oriented one.
//! - **Handles.** [`AddressableHeap::insert`] returns an opaque, `Copy`
//!   handle used to inspect, update, or delete that entry later. A handle is
//!   rejected once its entry is removed, its heap is cleared, or it is
//!   presented to a different heap instance. Key decreases are a separate
//!   capability, [`DecreaseKeyHeap`]: a handle-based heap that cannot
//!   restore heap order after a decrease simply does not implement it.
//! - **Melding.** [`MeldableHeap::meld`] and its addressable and
//!   double-ended counterparts efficiently absorb another heap of the same
//!   concrete type. A successful meld consumes the donor for further
//!   mutation; handles the donor already issued stay valid through the
//!   receiver.
//! - **Fallibility.** Ordinary heaps never fail to insert. Radix heaps in
//!   [`monotone`] are the exception: their constructors validate key bounds,
//!   and insertion enforces monotonicity, both reported through `Result`.
//!
//! # Relationship to JHeaps
//!
//! The implementation set and much of the behavioral test coverage are
//! derived from [JHeaps](https://github.com/d-michail/jheaps). The API
//! follows Rust's ownership, trait, and error-handling conventions rather
//! than reproducing the Java API literally.

pub mod array;
pub mod dag;
pub mod error;
pub mod monotone;
pub mod tree;

pub use error::{DecreaseKeyError, IncreaseKeyError, InvalidHandle};

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
    fn insert(&mut self, key: K, value: V);

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
    -> Result<(), error::IncreaseKeyError>;
}

/// A min-oriented heap whose entries are addressed by stable handles.
///
/// A handle is an opaque capability returned from [`Self::insert`]. Its
/// validity is checked by every handle operation; it becomes invalid when the
/// entry is removed or the heap is cleared. Handles cannot be used with
/// another heap.
pub trait AddressableHeap<K, V> {
    /// Opaque type that identifies a live entry in this heap.
    type Handle: Copy + Eq;

    /// Inserts an entry and returns its handle.
    fn insert(&mut self, key: K, value: V) -> Self::Handle;

    /// Returns the handle, key, and value of a minimum entry, if present.
    fn peek(&self) -> Option<(Self::Handle, &K, &V)>;

    /// Removes and returns a minimum entry, if present.
    fn pop(&mut self) -> Option<(K, V)>;

    /// Returns the key identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error if `handle` is stale or belongs to another heap.
    fn key(&self, handle: Self::Handle) -> Result<&K, error::InvalidHandle>;

    /// Returns the value identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error if `handle` is stale or belongs to another heap.
    fn value(&self, handle: Self::Handle) -> Result<&V, error::InvalidHandle>;

    /// Returns mutable access to the value identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error if `handle` is stale or belongs to another heap.
    fn value_mut(&mut self, handle: Self::Handle) -> Result<&mut V, error::InvalidHandle>;

    /// Removes and returns the entry identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error if `handle` is stale or belongs to another heap.
    fn delete(&mut self, handle: Self::Handle) -> Result<(K, V), error::InvalidHandle>;

    /// Returns the number of live entries.
    fn len(&self) -> usize;

    /// Removes all entries and invalidates every outstanding handle.
    fn clear(&mut self);

    /// Returns whether the heap contains no entries.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// An addressable heap that supports decreasing a live entry's key.
///
/// Every heap in this crate that tracks enough per-entry structure to
/// restore heap order after a key decrease implements this trait in addition
/// to [`AddressableHeap`]. A heap that cannot support the operation - for
/// example, [`tree::BinaryTreeSoftAddressableHeap`], whose corruption-bounded
/// structure does not track precise entry positions - simply does not
/// implement it, so attempting to decrease its keys is a compile-time error
/// rather than a runtime one.
pub trait DecreaseKeyHeap<K, V>: AddressableHeap<K, V> {
    /// Decreases the key identified by `handle`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid handle or a key with lower priority
    /// than the current one.
    fn decrease_key(&mut self, handle: Self::Handle, key: K)
    -> Result<(), error::DecreaseKeyError>;
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

/// Implements [`Heap<T>`] for `$ty<T, ()>` by forwarding to its
/// [`AddressableHeap<T, ()>`] implementation.
///
/// A blanket impl over every `H: AddressableHeap<T, ()>` is not possible on
/// stable Rust: it would conflict with the direct `Heap<T>` impls that
/// non-addressable heaps (which never implement `AddressableHeap`) provide
/// for themselves, since coherence checking cannot prove the two never
/// overlap without specialization. This macro keeps the forwarding logic
/// defined once while still emitting one concrete, non-overlapping impl per
/// invocation.
#[macro_export]
macro_rules! impl_heap_via_addressable {
    ($ty:ident) => {
        impl<T: Ord> $crate::Heap<T> for $ty<T, ()> {
            fn push(&mut self, value: T) {
                <Self as $crate::AddressableHeap<T, ()>>::insert(self, value, ());
            }

            fn peek(&self) -> Option<&T> {
                <Self as $crate::AddressableHeap<T, ()>>::peek(self).map(|(_, key, _)| key)
            }

            fn pop(&mut self) -> Option<T> {
                <Self as $crate::AddressableHeap<T, ()>>::pop(self).map(|(key, ())| key)
            }

            fn len(&self) -> usize {
                $crate::AddressableHeap::len(self)
            }

            fn clear(&mut self) {
                $crate::AddressableHeap::clear(self);
            }
        }
    };
}

/// Implements [`MeldableHeap<T>`] for `$ty<T, ()>` by forwarding to its
/// [`MeldableAddressableHeap<T, ()>`] implementation. See
/// [`impl_heap_via_addressable`] for why this is a macro rather than a
/// blanket impl.
#[macro_export]
macro_rules! impl_meldable_heap_via_addressable {
    ($ty:ident) => {
        impl<T: Ord> $crate::MeldableHeap<T> for $ty<T, ()> {
            type MeldError = <Self as $crate::MeldableAddressableHeap<T, ()>>::MeldError;

            fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
                $crate::MeldableAddressableHeap::meld(self, other)
            }
        }
    };
}

/// Implements [`DoubleEndedHeap<T>`] for `$ty<T, ()>` by forwarding to its
/// [`DoubleEndedAddressableHeap<T, ()>`] implementation. See
/// [`impl_heap_via_addressable`] for why this is a macro rather than a
/// blanket impl.
#[macro_export]
macro_rules! impl_double_ended_heap_via_addressable {
    ($ty:ident) => {
        impl<T: Ord> $crate::DoubleEndedHeap<T> for $ty<T, ()> {
            fn peek_max(&self) -> Option<&T> {
                <Self as $crate::DoubleEndedAddressableHeap<T, ()>>::peek_max(self)
                    .map(|(_, key, _)| key)
            }

            fn pop_max(&mut self) -> Option<T> {
                <Self as $crate::DoubleEndedAddressableHeap<T, ()>>::pop_max(self)
                    .map(|(key, ())| key)
            }
        }
    };
}

#[cfg(test)]
pub(crate) mod test_support {
    use core::cmp::Ordering;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct ReverseKey(pub(crate) i32);

    impl PartialOrd for ReverseKey {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for ReverseKey {
        fn cmp(&self, other: &Self) -> Ordering {
            other.0.cmp(&self.0)
        }
    }
}
