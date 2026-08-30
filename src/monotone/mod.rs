//! Monotone radix heaps for unsigned integers, finite floating-point keys,
//! and arbitrary-sized unsigned integers.
//!
//! A radix heap is efficient when items are removed in nondecreasing key
//! order, such as in Dijkstra's algorithm.  A key inserted after an item has
//! been removed must be no smaller than that last removed key.  Insertion and
//! key decreases report violations of these constraints through
//! [`RadixHeapError`] rather than panicking, so every radix heap implements
//! the fallible [`TryHeap`]/[`TryAddressableHeap`]/[`TryDecreaseKeyHeap`]
//! traits (and the matching inherent `try_push`/`try_insert`/`decrease_key`
//! methods) instead of the infallible [`Heap`](crate::Heap)/
//! [`AddressableHeap`](crate::AddressableHeap).
//!
//! Floating-point heaps use [`FiniteF64`], which rejects NaN and infinities
//! and implements the total ordering of [`f64::total_cmp`].
//!
//! # Generic fallible use
//!
//! ```
//! use rheaps::monotone::{RadixHeapError, U32RadixHeap};
//! use rheaps::TryHeap;
//!
//! fn insert_or_report<H>(heap: &mut H, key: u32) -> Result<(), H::InsertError>
//! where
//!     H: TryHeap<u32>,
//! {
//!     heap.try_push(key)
//! }
//!
//! let mut heap = U32RadixHeap::new(0, 10).unwrap();
//! insert_or_report(&mut heap, 3).unwrap();
//! assert_eq!(
//!     insert_or_report(&mut heap, 20),
//!     Err(RadixHeapError::KeyOutOfRange)
//! );
//! assert_eq!(heap.pop(), Some(3));
//! ```

use core::cmp::Ordering;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

pub use num_bigint::BigUint;

use crate::error::{DecreaseKeyError, InvalidHandle};
use crate::{TryAddressableHeap, TryDecreaseKeyHeap, TryHeap};

static NEXT_HEAP_ID: AtomicU64 = AtomicU64::new(1);

/// An error returned when constructing a [`FiniteF64`] from a non-finite
/// floating-point value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonFiniteF64;

impl fmt::Display for NonFiniteF64 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("floating-point key must be finite")
    }
}

impl std::error::Error for NonFiniteF64 {}

/// A finite `f64` with a total order.
///
/// Its order is [`f64::total_cmp`], so negative zero sorts before positive
/// zero. Construct one with [`FiniteF64::new`] or [`TryFrom<f64>`].
#[derive(Clone, Copy, Debug)]
pub struct FiniteF64(f64);

impl FiniteF64 {
    /// Creates a finite key, rejecting NaN and infinities.
    pub fn new(value: f64) -> Result<Self, NonFiniteF64> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(NonFiniteF64)
        }
    }

    /// Returns the wrapped floating-point value.
    #[must_use]
    pub const fn as_f64(self) -> f64 {
        self.0
    }

    /// Consumes the key and returns the wrapped floating-point value.
    #[must_use]
    pub const fn into_inner(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FiniteF64 {
    type Error = NonFiniteF64;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FiniteF64> for f64 {
    fn from(value: FiniteF64) -> Self {
        value.0
    }
}

impl PartialEq for FiniteF64 {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for FiniteF64 {}

impl PartialOrd for FiniteF64 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FiniteF64 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl core::hash::Hash for FiniteF64 {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

/// A key or bounds error reported by a monotone radix heap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadixHeapError {
    /// The supplied lower and upper bounds do not define a supported range.
    InvalidRange,
    /// A key was outside the bounds supplied when constructing the heap.
    KeyOutOfRange,
    /// A key was less than the most recently removed key.
    MonotonicityViolation,
    /// The arbitrary-sized key range would require more buckets than this
    /// platform can index.
    TooManyBuckets,
}

impl fmt::Display for RadixHeapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange => formatter.write_str("invalid radix heap key range"),
            Self::KeyOutOfRange => {
                formatter.write_str("key is outside the heap's configured range")
            }
            Self::MonotonicityViolation => {
                formatter.write_str("key is less than the last key removed from this monotone heap")
            }
            Self::TooManyBuckets => {
                formatter.write_str("radix heap key range requires too many buckets")
            }
        }
    }
}

impl std::error::Error for RadixHeapError {}

/// An error returned by an addressable radix heap key decrease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadixDecreaseKeyError {
    /// The handle was stale or belongs to another heap.
    InvalidHandle(InvalidHandle),
    /// The proposed key was greater than the entry's existing key.
    NotDecreased,
    /// The proposed key violated the radix heap's range or monotonicity
    /// restriction.
    Radix(RadixHeapError),
}

impl fmt::Display for RadixDecreaseKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHandle(error) => error.fmt(formatter),
            Self::NotDecreased => {
                formatter.write_str("new key must not be greater than the old key")
            }
            Self::Radix(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RadixDecreaseKeyError {}

impl From<RadixDecreaseKeyError> for DecreaseKeyError {
    fn from(error: RadixDecreaseKeyError) -> Self {
        match error {
            RadixDecreaseKeyError::InvalidHandle(error) => Self::InvalidHandle(error),
            RadixDecreaseKeyError::NotDecreased => Self::NotDecreased,
            RadixDecreaseKeyError::Radix(_) => Self::InvalidKey,
        }
    }
}

trait RadixKey: Ord + Clone {
    fn msd(&self, other: &Self) -> usize;
    fn bucket_count(minimum: &Self, maximum: &Self) -> Result<usize, RadixHeapError>;

    fn validate(&self) -> Result<(), RadixHeapError> {
        Ok(())
    }

    fn validate_bounds(minimum: &Self, maximum: &Self) -> Result<(), RadixHeapError> {
        minimum.validate()?;
        maximum.validate()?;
        if minimum > maximum {
            return Err(RadixHeapError::InvalidRange);
        }
        Ok(())
    }
}

impl RadixKey for u32 {
    fn msd(&self, other: &Self) -> usize {
        (u32::BITS - 1 - (*self ^ *other).leading_zeros()) as usize
    }

    fn bucket_count(minimum: &Self, maximum: &Self) -> Result<usize, RadixHeapError> {
        let range = *maximum - *minimum;
        Ok((u32::BITS - range.leading_zeros()) as usize + 2)
    }
}

impl RadixKey for u64 {
    fn msd(&self, other: &Self) -> usize {
        (u64::BITS - 1 - (*self ^ *other).leading_zeros()) as usize
    }

    fn bucket_count(minimum: &Self, maximum: &Self) -> Result<usize, RadixHeapError> {
        let range = *maximum - *minimum;
        Ok((u64::BITS - range.leading_zeros()) as usize + 2)
    }
}

impl RadixKey for BigUint {
    fn msd(&self, other: &Self) -> usize {
        let difference = self ^ other;
        usize::try_from(difference.bits() - 1).expect("key bits fit in addressable memory")
    }

    fn bucket_count(minimum: &Self, maximum: &Self) -> Result<usize, RadixHeapError> {
        let range = maximum - minimum;
        usize::try_from(range.bits())
            .ok()
            .and_then(|bits| bits.checked_add(2))
            .ok_or(RadixHeapError::TooManyBuckets)
    }
}

fn float_rank(key: FiniteF64) -> u64 {
    let bits = key.0.to_bits();
    if bits >> 63 == 0 {
        bits | (1_u64 << 63)
    } else {
        !bits
    }
}

impl RadixKey for FiniteF64 {
    fn msd(&self, other: &Self) -> usize {
        (u64::BITS - 1 - (float_rank(*self) ^ float_rank(*other)).leading_zeros()) as usize
    }

    fn bucket_count(minimum: &Self, maximum: &Self) -> Result<usize, RadixHeapError> {
        let range = float_rank(*maximum) - float_rank(*minimum);
        Ok((u64::BITS - range.leading_zeros()) as usize + 2)
    }

    fn validate_bounds(minimum: &Self, maximum: &Self) -> Result<(), RadixHeapError> {
        if minimum.0 < 0.0 || minimum > maximum {
            return Err(RadixHeapError::InvalidRange);
        }
        Ok(())
    }
}

struct RadixHeapCore<K> {
    buckets: Vec<Vec<K>>,
    len: usize,
    last_deleted_key: K,
    minimum_key: K,
    maximum_key: K,
    current_minimum: Option<(usize, usize)>,
}

impl<K: RadixKey> RadixHeapCore<K> {
    fn new(minimum_key: K, maximum_key: K) -> Result<Self, RadixHeapError> {
        K::validate_bounds(&minimum_key, &maximum_key)?;
        let bucket_count = K::bucket_count(&minimum_key, &maximum_key)?;
        Ok(Self {
            buckets: (0..bucket_count).map(|_| Vec::new()).collect(),
            len: 0,
            last_deleted_key: minimum_key.clone(),
            minimum_key,
            maximum_key,
            current_minimum: None,
        })
    }

    fn check_key(&self, key: &K) -> Result<(), RadixHeapError> {
        key.validate()?;
        if key < &self.minimum_key || key > &self.maximum_key {
            return Err(RadixHeapError::KeyOutOfRange);
        }
        if key < &self.last_deleted_key {
            return Err(RadixHeapError::MonotonicityViolation);
        }
        Ok(())
    }

    fn bucket_for(&self, key: &K) -> usize {
        if key == &self.last_deleted_key {
            0
        } else {
            1 + key.msd(&self.last_deleted_key).min(self.buckets.len() - 2)
        }
    }

    fn try_push(&mut self, key: K) -> Result<(), RadixHeapError> {
        self.check_key(&key)?;
        let replace_minimum = match self.current_minimum {
            Some((bucket, position)) => key < self.buckets[bucket][position],
            None => true,
        };
        let bucket = self.bucket_for(&key);
        self.buckets[bucket].push(key);
        if replace_minimum {
            self.current_minimum = Some((bucket, self.buckets[bucket].len() - 1));
        }
        self.len += 1;
        Ok(())
    }

    fn peek(&self) -> Option<&K> {
        self.current_minimum
            .map(|(bucket, position)| &self.buckets[bucket][position])
    }

    fn pop(&mut self) -> Option<K> {
        let (bucket, position) = self.current_minimum.take()?;
        let result = if bucket == 0 {
            self.buckets[bucket].swap_remove(position)
        } else {
            let mut values = core::mem::take(&mut self.buckets[bucket]);
            let result = values.swap_remove(position);
            self.last_deleted_key = result.clone();
            for value in values {
                let new_bucket = self.bucket_for(&value);
                debug_assert!(new_bucket < bucket);
                self.buckets[new_bucket].push(value);
            }
            result
        };

        self.last_deleted_key = result.clone();
        self.len -= 1;
        if self.len != 0 {
            self.cache_minimum_from(0);
        }
        Some(result)
    }

    fn cache_minimum_from(&mut self, first_bucket: usize) {
        let bucket = (first_bucket..self.buckets.len())
            .find(|&index| !self.buckets[index].is_empty())
            .expect("a non-empty radix heap has a non-empty bucket");
        let mut position = 0;
        for candidate in 1..self.buckets[bucket].len() {
            if self.buckets[bucket][candidate] < self.buckets[bucket][position] {
                position = candidate;
            }
        }
        self.current_minimum = Some((bucket, position));
    }

    fn clear(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.len = 0;
        self.last_deleted_key = self.minimum_key.clone();
        self.current_minimum = None;
    }
}

/// An opaque capability identifying a live entry in an addressable radix heap.
///
/// A handle is valid only for the heap that returned it, and becomes stale
/// after its entry is removed or the heap is cleared.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RadixHandle {
    heap_id: u64,
    slot: usize,
    generation: u64,
}

struct AddressableEntry<K, V> {
    key: K,
    value: V,
    bucket: usize,
    position: usize,
}

struct AddressableSlot<K, V> {
    entry: Option<AddressableEntry<K, V>>,
    generation: u64,
}

struct AddressableRadixHeapCore<K, V> {
    buckets: Vec<Vec<usize>>,
    slots: Vec<AddressableSlot<K, V>>,
    free_slots: Vec<usize>,
    len: usize,
    last_deleted_key: K,
    minimum_key: K,
    maximum_key: K,
    current_minimum: Option<usize>,
    heap_id: u64,
}

impl<K: RadixKey, V> AddressableRadixHeapCore<K, V> {
    fn new(minimum_key: K, maximum_key: K) -> Result<Self, RadixHeapError> {
        K::validate_bounds(&minimum_key, &maximum_key)?;
        let bucket_count = K::bucket_count(&minimum_key, &maximum_key)?;
        Ok(Self {
            buckets: (0..bucket_count).map(|_| Vec::new()).collect(),
            slots: Vec::new(),
            free_slots: Vec::new(),
            len: 0,
            last_deleted_key: minimum_key.clone(),
            minimum_key,
            maximum_key,
            current_minimum: None,
            heap_id: next_heap_id(),
        })
    }

    fn check_key(&self, key: &K) -> Result<(), RadixHeapError> {
        key.validate()?;
        if key < &self.minimum_key || key > &self.maximum_key {
            return Err(RadixHeapError::KeyOutOfRange);
        }
        if key < &self.last_deleted_key {
            return Err(RadixHeapError::MonotonicityViolation);
        }
        Ok(())
    }

    fn bucket_for(&self, key: &K) -> usize {
        if key == &self.last_deleted_key {
            0
        } else {
            1 + key.msd(&self.last_deleted_key).min(self.buckets.len() - 2)
        }
    }

    fn handle(&self, slot: usize) -> RadixHandle {
        RadixHandle {
            heap_id: self.heap_id,
            slot,
            generation: self.slots[slot].generation,
        }
    }

    fn validate(&self, handle: RadixHandle) -> Result<usize, InvalidHandle> {
        if handle.heap_id != self.heap_id {
            return Err(InvalidHandle::ForeignHeap);
        }
        let Some(slot) = self.slots.get(handle.slot) else {
            return Err(InvalidHandle::Stale);
        };
        if slot.generation != handle.generation || slot.entry.is_none() {
            return Err(InvalidHandle::Stale);
        }
        Ok(handle.slot)
    }

    fn try_insert(&mut self, key: K, value: V) -> Result<RadixHandle, RadixHeapError> {
        self.check_key(&key)?;
        let replace_minimum = match self.current_minimum {
            Some(slot) => key < self.slots[slot].entry.as_ref().expect("live minimum").key,
            None => true,
        };
        let bucket = self.bucket_for(&key);
        let slot = match self.free_slots.pop() {
            Some(slot) => slot,
            None => {
                let slot = self.slots.len();
                self.slots.push(AddressableSlot {
                    entry: None,
                    generation: 0,
                });
                slot
            }
        };
        let position = self.buckets[bucket].len();
        self.buckets[bucket].push(slot);
        self.slots[slot].entry = Some(AddressableEntry {
            key,
            value,
            bucket,
            position,
        });
        if replace_minimum {
            self.current_minimum = Some(slot);
        }
        self.len += 1;
        Ok(self.handle(slot))
    }

    fn peek(&self) -> Option<(RadixHandle, &K, &V)> {
        let slot = self.current_minimum?;
        let handle = self.handle(slot);
        let entry = self.slots[slot].entry.as_ref().expect("live minimum");
        Some((handle, &entry.key, &entry.value))
    }

    fn key(&self, handle: RadixHandle) -> Result<&K, InvalidHandle> {
        let slot = self.validate(handle)?;
        Ok(&self.slots[slot]
            .entry
            .as_ref()
            .expect("validated entry")
            .key)
    }

    fn value(&self, handle: RadixHandle) -> Result<&V, InvalidHandle> {
        let slot = self.validate(handle)?;
        Ok(&self.slots[slot]
            .entry
            .as_ref()
            .expect("validated entry")
            .value)
    }

    fn value_mut(&mut self, handle: RadixHandle) -> Result<&mut V, InvalidHandle> {
        let slot = self.validate(handle)?;
        Ok(&mut self.slots[slot]
            .entry
            .as_mut()
            .expect("validated entry")
            .value)
    }

    fn remove_from_bucket(&mut self, slot: usize) {
        let (bucket, position) = {
            let entry = self.slots[slot].entry.as_ref().expect("live entry");
            (entry.bucket, entry.position)
        };
        let removed = self.buckets[bucket].swap_remove(position);
        debug_assert_eq!(removed, slot);
        if let Some(&moved) = self.buckets[bucket].get(position) {
            self.slots[moved]
                .entry
                .as_mut()
                .expect("bucket entries are live")
                .position = position;
        }
    }

    fn take_slot(&mut self, slot: usize) -> AddressableEntry<K, V> {
        let entry = self.slots[slot].entry.take().expect("live entry");
        self.slots[slot].generation = self.slots[slot].generation.wrapping_add(1);
        self.free_slots.push(slot);
        entry
    }

    fn add_to_bucket(&mut self, slot: usize, bucket: usize) {
        let position = self.buckets[bucket].len();
        self.buckets[bucket].push(slot);
        let entry = self.slots[slot].entry.as_mut().expect("live entry");
        entry.bucket = bucket;
        entry.position = position;
    }

    fn pop(&mut self) -> Option<(K, V)> {
        let slot = self.current_minimum.take()?;
        let bucket = self.slots[slot]
            .entry
            .as_ref()
            .expect("live minimum")
            .bucket;

        let result = if bucket == 0 {
            self.remove_from_bucket(slot);
            self.take_slot(slot)
        } else {
            let members = core::mem::take(&mut self.buckets[bucket]);
            let result = self.take_slot(slot);
            self.last_deleted_key = result.key.clone();
            for member in members {
                if member != slot {
                    let new_bucket = self.bucket_for(
                        &self.slots[member]
                            .entry
                            .as_ref()
                            .expect("bucket entries are live")
                            .key,
                    );
                    debug_assert!(new_bucket < bucket);
                    self.add_to_bucket(member, new_bucket);
                }
            }
            result
        };

        self.last_deleted_key = result.key.clone();
        self.len -= 1;
        if self.len != 0 {
            self.cache_minimum_from(0);
        }
        Some((result.key, result.value))
    }

    fn cache_minimum_from(&mut self, first_bucket: usize) {
        let bucket = (first_bucket..self.buckets.len())
            .find(|&index| !self.buckets[index].is_empty())
            .expect("a non-empty radix heap has a non-empty bucket");
        let mut minimum = self.buckets[bucket][0];
        for &candidate in &self.buckets[bucket][1..] {
            let candidate_key = &self.slots[candidate]
                .entry
                .as_ref()
                .expect("bucket entries are live")
                .key;
            let minimum_key = &self.slots[minimum]
                .entry
                .as_ref()
                .expect("bucket entries are live")
                .key;
            if candidate_key < minimum_key {
                minimum = candidate;
            }
        }
        self.current_minimum = Some(minimum);
    }

    fn delete(&mut self, handle: RadixHandle) -> Result<(K, V), InvalidHandle> {
        let slot = self.validate(handle)?;
        if self.current_minimum == Some(slot) {
            return Ok(self.pop().expect("a live minimum can be removed"));
        }
        self.remove_from_bucket(slot);
        let entry = self.take_slot(slot);
        self.len -= 1;
        Ok((entry.key, entry.value))
    }

    fn try_decrease_key(
        &mut self,
        handle: RadixHandle,
        key: K,
    ) -> Result<(), RadixDecreaseKeyError> {
        let slot = self
            .validate(handle)
            .map_err(RadixDecreaseKeyError::InvalidHandle)?;
        self.check_key(&key).map_err(RadixDecreaseKeyError::Radix)?;
        if key
            > self.slots[slot]
                .entry
                .as_ref()
                .expect("validated entry")
                .key
        {
            return Err(RadixDecreaseKeyError::NotDecreased);
        }

        let bucket = self.bucket_for(&key);
        let old_bucket = self.slots[slot]
            .entry
            .as_ref()
            .expect("validated entry")
            .bucket;
        let replace_minimum = match self.current_minimum {
            Some(minimum) if minimum != slot => {
                key < self.slots[minimum]
                    .entry
                    .as_ref()
                    .expect("live minimum")
                    .key
            }
            _ => true,
        };

        if bucket != old_bucket {
            self.remove_from_bucket(slot);
        }
        self.slots[slot]
            .entry
            .as_mut()
            .expect("validated entry")
            .key = key;
        if bucket != old_bucket {
            self.add_to_bucket(slot, bucket);
        }
        if replace_minimum {
            self.current_minimum = Some(slot);
        }
        Ok(())
    }

    fn clear(&mut self) {
        for slot in &mut self.slots {
            if slot.entry.take().is_some() {
                slot.generation = slot.generation.wrapping_add(1);
            }
        }
        self.free_slots.clear();
        self.free_slots.extend(0..self.slots.len());
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.len = 0;
        self.last_deleted_key = self.minimum_key.clone();
        self.current_minimum = None;
    }
}

fn next_heap_id() -> u64 {
    let id = NEXT_HEAP_ID.fetch_add(1, AtomicOrdering::Relaxed);
    if id == 0 {
        NEXT_HEAP_ID.fetch_add(1, AtomicOrdering::Relaxed)
    } else {
        id
    }
}

macro_rules! define_radix_heap {
    ($name:ident, $key:ty, $documentation:literal) => {
        #[doc = $documentation]
        ///
        /// This heap uses radix buckets rather than a comparison heap. Keys
        /// must remain within its construction bounds and cannot move below
        /// the most recently removed key.
        pub struct $name {
            core: RadixHeapCore<$key>,
        }

        impl $name {
            /// Creates an empty radix heap with inclusive key bounds.
            pub fn new(minimum_key: $key, maximum_key: $key) -> Result<Self, RadixHeapError> {
                Ok(Self {
                    core: RadixHeapCore::new(minimum_key, maximum_key)?,
                })
            }

            /// Alias for [`Self::new`].
            pub fn with_bounds(
                minimum_key: $key,
                maximum_key: $key,
            ) -> Result<Self, RadixHeapError> {
                Self::new(minimum_key, maximum_key)
            }

            /// Returns the inclusive lower key bound.
            #[must_use]
            pub fn minimum_key(&self) -> &$key {
                &self.core.minimum_key
            }

            /// Returns the inclusive upper key bound.
            #[must_use]
            pub fn maximum_key(&self) -> &$key {
                &self.core.maximum_key
            }

            /// Returns the most recently removed key, or the lower bound if
            /// no key has yet been removed or the heap was cleared.
            #[must_use]
            pub fn last_deleted_key(&self) -> &$key {
                &self.core.last_deleted_key
            }

            /// Returns the fixed number of radix buckets.
            #[must_use]
            pub fn bucket_count(&self) -> usize {
                self.core.buckets.len()
            }

            /// Inserts `key`.
            ///
            /// Returns an error for out-of-range keys and keys below the last
            /// removed key.
            pub fn try_push(&mut self, key: $key) -> Result<(), RadixHeapError> {
                self.core.try_push(key)
            }

            /// Returns the current minimum key.
            #[must_use]
            pub fn peek(&self) -> Option<&$key> {
                self.core.peek()
            }

            /// Removes and returns the current minimum key.
            pub fn pop(&mut self) -> Option<$key> {
                self.core.pop()
            }

            /// Returns the number of keys in the heap.
            #[must_use]
            pub fn len(&self) -> usize {
                self.core.len
            }

            /// Returns whether the heap contains no keys.
            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.core.len == 0
            }

            /// Removes all keys and resets the monotonicity constraint.
            pub fn clear(&mut self) {
                self.core.clear();
            }
        }

        impl TryHeap<$key> for $name {
            type InsertError = RadixHeapError;

            fn try_push(&mut self, value: $key) -> Result<(), Self::InsertError> {
                Self::try_push(self, value)
            }

            fn peek(&self) -> Option<&$key> {
                Self::peek(self)
            }

            fn pop(&mut self) -> Option<$key> {
                Self::pop(self)
            }

            fn len(&self) -> usize {
                Self::len(self)
            }

            fn clear(&mut self) {
                Self::clear(self);
            }
        }
    };
}

define_radix_heap!(U32RadixHeap, u32, "A monotone radix heap for `u32` keys.");
define_radix_heap!(U64RadixHeap, u64, "A monotone radix heap for `u64` keys.");
define_radix_heap!(
    F64RadixHeap,
    FiniteF64,
    "A monotone radix heap for finite, non-negative [`FiniteF64`] keys."
);
define_radix_heap!(
    BigUintRadixHeap,
    BigUint,
    "A monotone radix heap for arbitrary-sized unsigned [`BigUint`] keys."
);

macro_rules! define_addressable_radix_heap {
    ($name:ident, $key:ty, $documentation:literal) => {
        #[doc = $documentation]
        ///
        /// Handles are checked for foreign and stale use. As with every radix
        /// heap in this module, fallible insertion and key-update methods
        /// enforce range and monotonicity restrictions.
        pub struct $name<V> {
            core: AddressableRadixHeapCore<$key, V>,
        }

        impl<V> $name<V> {
            /// Creates an empty addressable radix heap with inclusive bounds.
            pub fn new(minimum_key: $key, maximum_key: $key) -> Result<Self, RadixHeapError> {
                Ok(Self {
                    core: AddressableRadixHeapCore::new(minimum_key, maximum_key)?,
                })
            }

            /// Alias for [`Self::new`].
            pub fn with_bounds(
                minimum_key: $key,
                maximum_key: $key,
            ) -> Result<Self, RadixHeapError> {
                Self::new(minimum_key, maximum_key)
            }

            /// Returns the inclusive lower key bound.
            #[must_use]
            pub fn minimum_key(&self) -> &$key {
                &self.core.minimum_key
            }

            /// Returns the inclusive upper key bound.
            #[must_use]
            pub fn maximum_key(&self) -> &$key {
                &self.core.maximum_key
            }

            /// Returns the most recently removed key, or the lower bound if
            /// no key has yet been removed or the heap was cleared.
            #[must_use]
            pub fn last_deleted_key(&self) -> &$key {
                &self.core.last_deleted_key
            }

            /// Returns the fixed number of radix buckets.
            #[must_use]
            pub fn bucket_count(&self) -> usize {
                self.core.buckets.len()
            }

            /// Inserts a key-value entry and returns its handle.
            pub fn try_insert(
                &mut self,
                key: $key,
                value: V,
            ) -> Result<RadixHandle, RadixHeapError> {
                self.core.try_insert(key, value)
            }

            /// Returns the handle, key, and value of the minimum entry.
            #[must_use]
            pub fn peek(&self) -> Option<(RadixHandle, &$key, &V)> {
                self.core.peek()
            }

            /// Removes and returns the minimum entry.
            pub fn pop(&mut self) -> Option<($key, V)> {
                self.core.pop()
            }

            /// Returns the key addressed by `handle`.
            pub fn key(&self, handle: RadixHandle) -> Result<&$key, InvalidHandle> {
                self.core.key(handle)
            }

            /// Returns the value addressed by `handle`.
            pub fn value(&self, handle: RadixHandle) -> Result<&V, InvalidHandle> {
                self.core.value(handle)
            }

            /// Returns mutable access to the value addressed by `handle`.
            pub fn value_mut(&mut self, handle: RadixHandle) -> Result<&mut V, InvalidHandle> {
                self.core.value_mut(handle)
            }

            /// Decreases an entry's key while preserving the monotonicity
            /// restriction.
            pub fn decrease_key(
                &mut self,
                handle: RadixHandle,
                key: $key,
            ) -> Result<(), RadixDecreaseKeyError> {
                self.core.try_decrease_key(handle, key)
            }

            /// Removes and returns the entry addressed by `handle`.
            pub fn delete(&mut self, handle: RadixHandle) -> Result<($key, V), InvalidHandle> {
                self.core.delete(handle)
            }

            /// Returns the number of live entries.
            #[must_use]
            pub fn len(&self) -> usize {
                self.core.len
            }

            /// Returns whether the heap contains no entries.
            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.core.len == 0
            }

            /// Removes all entries, invalidates their handles, and resets the
            /// monotonicity constraint.
            pub fn clear(&mut self) {
                self.core.clear();
            }
        }

        impl<V> TryAddressableHeap<$key, V> for $name<V> {
            type Handle = RadixHandle;
            type InsertError = RadixHeapError;

            fn try_insert(
                &mut self,
                key: $key,
                value: V,
            ) -> Result<Self::Handle, Self::InsertError> {
                Self::try_insert(self, key, value)
            }

            fn peek(&self) -> Option<(Self::Handle, &$key, &V)> {
                Self::peek(self)
            }

            fn pop(&mut self) -> Option<($key, V)> {
                Self::pop(self)
            }

            fn key(&self, handle: Self::Handle) -> Result<&$key, InvalidHandle> {
                Self::key(self, handle)
            }

            fn value(&self, handle: Self::Handle) -> Result<&V, InvalidHandle> {
                Self::value(self, handle)
            }

            fn value_mut(&mut self, handle: Self::Handle) -> Result<&mut V, InvalidHandle> {
                Self::value_mut(self, handle)
            }

            fn delete(&mut self, handle: Self::Handle) -> Result<($key, V), InvalidHandle> {
                Self::delete(self, handle)
            }

            fn len(&self) -> usize {
                Self::len(self)
            }

            fn clear(&mut self) {
                Self::clear(self);
            }
        }

        impl<V> TryDecreaseKeyHeap<$key, V> for $name<V> {
            type DecreaseKeyError = RadixDecreaseKeyError;

            fn decrease_key(
                &mut self,
                handle: Self::Handle,
                key: $key,
            ) -> Result<(), Self::DecreaseKeyError> {
                Self::decrease_key(self, handle, key)
            }
        }
    };
}

define_addressable_radix_heap!(
    U32RadixAddressableHeap,
    u32,
    "An addressable monotone radix heap for `u32` keys."
);
define_addressable_radix_heap!(
    U64RadixAddressableHeap,
    u64,
    "An addressable monotone radix heap for `u64` keys."
);
define_addressable_radix_heap!(
    F64RadixAddressableHeap,
    FiniteF64,
    "An addressable monotone radix heap for finite, non-negative [`FiniteF64`] keys."
);
define_addressable_radix_heap!(
    BigUintRadixAddressableHeap,
    BigUint,
    "An addressable monotone radix heap for arbitrary-sized unsigned [`BigUint`] keys."
);

#[cfg(test)]
mod tests;
