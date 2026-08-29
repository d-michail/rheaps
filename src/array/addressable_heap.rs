use core::cmp::Ordering;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::AddressableHeap;
use crate::array::{Comparator, InvalidDegree, NaturalOrder};

const DEFAULT_HEAP_CAPACITY: usize = 16;
static NEXT_HEAP_ID: AtomicU64 = AtomicU64::new(1);

/// An opaque capability that identifies an entry in an addressable heap.
///
/// Handles are `Copy`, but they are only valid in the heap that created them.
/// They become invalid after their entry is removed or after [`AddressableHeap::clear`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AddressableHandle {
    heap_id: u64,
    slot: usize,
    generation: u64,
}

/// The reason an addressable heap rejected a handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidHandle {
    /// The handle was created by a different heap.
    ForeignHeap,
    /// The entry was removed or the heap was cleared.
    Stale,
}

impl fmt::Display for InvalidHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignHeap => formatter.write_str("handle belongs to another heap"),
            Self::Stale => formatter.write_str("handle no longer identifies a live entry"),
        }
    }
}

impl std::error::Error for InvalidHandle {}

/// An error returned when decreasing an addressable entry's key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecreaseKeyError {
    /// The handle was not valid for this heap.
    InvalidHandle(InvalidHandle),
    /// The proposed key has lower priority than the existing key.
    NotDecreased,
    /// The proposed key violates an implementation-specific key restriction.
    ///
    /// For example, radix heaps require keys to be no less than their last
    /// deleted key.
    InvalidKey,
    /// The heap deliberately does not support key decreases.
    Unsupported,
}

impl fmt::Display for DecreaseKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHandle(error) => error.fmt(formatter),
            Self::NotDecreased => {
                formatter.write_str("new key must not be greater than the old key")
            }
            Self::InvalidKey => formatter.write_str("new key violates the heap's key restrictions"),
            Self::Unsupported => {
                formatter.write_str("key decreases are not supported by this heap")
            }
        }
    }
}

impl std::error::Error for DecreaseKeyError {}

/// An error returned when increasing an entry's key in a double-ended
/// addressable heap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncreaseKeyError {
    /// The handle was not valid for this heap.
    InvalidHandle(InvalidHandle),
    /// The proposed key has higher priority than the existing key.
    NotIncreased,
}

impl fmt::Display for IncreaseKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHandle(error) => error.fmt(formatter),
            Self::NotIncreased => formatter.write_str("new key must not be less than the old key"),
        }
    }
}

impl std::error::Error for IncreaseKeyError {}

struct Entry<K, V> {
    key: K,
    value: V,
    slot: usize,
}

struct Slot {
    index: Option<usize>,
    generation: u64,
}

struct AddressableCore<K, V, C> {
    entries: Vec<Entry<K, V>>,
    slots: Vec<Slot>,
    free_slots: Vec<usize>,
    compare: C,
    heap_id: u64,
}

impl<K, V, C> AddressableCore<K, V, C>
where
    C: Comparator<K>,
{
    fn new(capacity: usize, compare: C) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            slots: Vec::with_capacity(capacity),
            free_slots: Vec::new(),
            compare,
            heap_id: next_heap_id(),
        }
    }

    fn from_vec(entries: Vec<(K, V)>, degree: usize, compare: C) -> Self {
        let heap_id = next_heap_id();
        let mut slots = Vec::with_capacity(entries.len());
        let mut heap_entries = Vec::with_capacity(entries.len());
        for (index, (key, value)) in entries.into_iter().enumerate() {
            slots.push(Slot {
                index: Some(index),
                generation: 0,
            });
            heap_entries.push(Entry {
                key,
                value,
                slot: index,
            });
        }
        let mut heap = Self {
            entries: heap_entries,
            slots,
            free_slots: Vec::new(),
            compare,
            heap_id,
        };
        heap.heapify(degree);
        heap
    }

    fn comparator(&self) -> &C {
        &self.compare
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn handle_for_slot(&self, slot: usize) -> AddressableHandle {
        AddressableHandle {
            heap_id: self.heap_id,
            slot,
            generation: self.slots[slot].generation,
        }
    }

    fn validate(&self, handle: AddressableHandle) -> Result<usize, InvalidHandle> {
        if handle.heap_id != self.heap_id {
            return Err(InvalidHandle::ForeignHeap);
        }
        let Some(slot) = self.slots.get(handle.slot) else {
            return Err(InvalidHandle::Stale);
        };
        if slot.generation != handle.generation {
            return Err(InvalidHandle::Stale);
        }
        slot.index.ok_or(InvalidHandle::Stale)
    }

    fn push(&mut self, key: K, value: V, degree: usize) -> AddressableHandle {
        let slot = match self.free_slots.pop() {
            Some(slot) => {
                self.slots[slot].index = Some(self.entries.len());
                slot
            }
            None => {
                let slot = self.slots.len();
                self.slots.push(Slot {
                    index: Some(self.entries.len()),
                    generation: 0,
                });
                slot
            }
        };
        self.entries.push(Entry { key, value, slot });
        self.sift_up(self.entries.len() - 1, degree);
        self.handle_for_slot(slot)
    }

    fn peek(&self) -> Option<(AddressableHandle, &K, &V)> {
        self.entries
            .first()
            .map(|entry| (self.handle_for_slot(entry.slot), &entry.key, &entry.value))
    }

    fn pop(&mut self, degree: usize) -> Option<(K, V)> {
        if self.entries.is_empty() {
            return None;
        }
        let entry = self.remove_at(0);
        if !self.entries.is_empty() {
            self.sift_down(0, degree);
        }
        Some((entry.key, entry.value))
    }

    fn key(&self, handle: AddressableHandle) -> Result<&K, InvalidHandle> {
        let index = self.validate(handle)?;
        Ok(&self.entries[index].key)
    }

    fn value(&self, handle: AddressableHandle) -> Result<&V, InvalidHandle> {
        let index = self.validate(handle)?;
        Ok(&self.entries[index].value)
    }

    fn set_value(&mut self, handle: AddressableHandle, value: V) -> Result<(), InvalidHandle> {
        let index = self.validate(handle)?;
        self.entries[index].value = value;
        Ok(())
    }

    fn decrease_key(
        &mut self,
        handle: AddressableHandle,
        key: K,
        degree: usize,
    ) -> Result<(), DecreaseKeyError> {
        let index = self
            .validate(handle)
            .map_err(DecreaseKeyError::InvalidHandle)?;
        if self.compare.compare(&key, &self.entries[index].key) == Ordering::Greater {
            return Err(DecreaseKeyError::NotDecreased);
        }
        self.entries[index].key = key;
        self.sift_up(index, degree);
        Ok(())
    }

    fn delete(
        &mut self,
        handle: AddressableHandle,
        degree: usize,
    ) -> Result<(K, V), InvalidHandle> {
        let index = self.validate(handle)?;
        let entry = self.remove_at(index);
        if index < self.entries.len() {
            self.restore_at(index, degree);
        }
        Ok((entry.key, entry.value))
    }

    fn clear(&mut self) {
        while let Some(entry) = self.entries.pop() {
            self.invalidate_slot(entry.slot);
        }
    }

    fn handles(&self) -> impl Iterator<Item = AddressableHandle> + '_ {
        self.entries
            .iter()
            .map(|entry| self.handle_for_slot(entry.slot))
    }

    fn remove_at(&mut self, index: usize) -> Entry<K, V> {
        let entry = self.entries.swap_remove(index);
        self.invalidate_slot(entry.slot);
        if let Some(moved) = self.entries.get(index) {
            self.slots[moved.slot].index = Some(index);
        }
        entry
    }

    fn invalidate_slot(&mut self, slot_index: usize) {
        let slot = &mut self.slots[slot_index];
        slot.index = None;
        slot.generation = slot.generation.wrapping_add(1);
        self.free_slots.push(slot_index);
    }

    fn heapify(&mut self, degree: usize) {
        if self.entries.len() < 2 {
            return;
        }
        let last_parent = (self.entries.len() - 2) / degree;
        for index in (0..=last_parent).rev() {
            self.sift_down(index, degree);
        }
    }

    fn restore_at(&mut self, index: usize, degree: usize) {
        if index > 0 {
            let parent = (index - 1) / degree;
            if self
                .compare
                .compare(&self.entries[index].key, &self.entries[parent].key)
                == Ordering::Less
            {
                self.sift_up(index, degree);
                return;
            }
        }
        self.sift_down(index, degree);
    }

    fn sift_up(&mut self, mut index: usize, degree: usize) {
        while index > 0 {
            let parent = (index - 1) / degree;
            if self
                .compare
                .compare(&self.entries[parent].key, &self.entries[index].key)
                != Ordering::Greater
            {
                break;
            }
            self.swap_entries(parent, index);
            index = parent;
        }
    }

    fn sift_down(&mut self, mut index: usize, degree: usize) {
        loop {
            let first_child = index
                .checked_mul(degree)
                .and_then(|value| value.checked_add(1))
                .unwrap_or(self.entries.len());
            if first_child >= self.entries.len() {
                return;
            }
            let end = first_child.saturating_add(degree).min(self.entries.len());
            let mut smallest = first_child;
            for child in first_child + 1..end {
                if self
                    .compare
                    .compare(&self.entries[child].key, &self.entries[smallest].key)
                    == Ordering::Less
                {
                    smallest = child;
                }
            }
            if self
                .compare
                .compare(&self.entries[index].key, &self.entries[smallest].key)
                != Ordering::Greater
            {
                return;
            }
            self.swap_entries(index, smallest);
            index = smallest;
        }
    }

    fn swap_entries(&mut self, left: usize, right: usize) {
        self.entries.swap(left, right);
        self.slots[self.entries[left].slot].index = Some(left);
        self.slots[self.entries[right].slot].index = Some(right);
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

/// An array-backed binary min-heap with stable, checked entry handles.
///
/// Insertion, removal, deletion by handle, and key decreases are `O(log n)`;
/// inspecting the minimum is `O(1)`.
pub struct BinaryArrayAddressableHeap<K, V, C = NaturalOrder> {
    inner: AddressableCore<K, V, C>,
}

impl<K: Ord, V> BinaryArrayAddressableHeap<K, V> {
    /// Creates an empty heap using the natural ordering of keys.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_HEAP_CAPACITY)
    }

    /// Creates an empty heap with storage for at least `capacity` entries.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: AddressableCore::new(capacity, NaturalOrder),
        }
    }

    /// Builds a heap from key-value pairs in linear time.
    #[must_use]
    pub fn from_vec(entries: Vec<(K, V)>) -> Self {
        Self {
            inner: AddressableCore::from_vec(entries, 2, NaturalOrder),
        }
    }
}

impl<K: Ord, V> Default for BinaryArrayAddressableHeap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, C> BinaryArrayAddressableHeap<K, V, C>
where
    C: Comparator<K>,
{
    /// Creates an empty heap ordered by `compare`.
    #[must_use]
    pub fn with_comparator(compare: C) -> Self {
        Self::with_capacity_and_comparator(DEFAULT_HEAP_CAPACITY, compare)
    }

    /// Creates an empty heap with storage for at least `capacity` entries.
    #[must_use]
    pub fn with_capacity_and_comparator(capacity: usize, compare: C) -> Self {
        Self {
            inner: AddressableCore::new(capacity, compare),
        }
    }

    /// Builds a heap from key-value pairs in linear time using `compare`.
    #[must_use]
    pub fn from_vec_by(entries: Vec<(K, V)>, compare: C) -> Self {
        Self {
            inner: AddressableCore::from_vec(entries, 2, compare),
        }
    }

    /// Returns the comparator used to order keys.
    #[must_use]
    pub fn comparator(&self) -> &C {
        self.inner.comparator()
    }

    /// Inserts an entry and returns a handle that addresses it while live.
    pub fn push(&mut self, key: K, value: V) -> AddressableHandle {
        self.inner.push(key, value, 2)
    }

    /// Alias for [`Self::push`], matching JHeaps terminology.
    pub fn insert(&mut self, key: K, value: V) -> AddressableHandle {
        self.push(key, value)
    }

    /// Returns the handle, key, and value of a minimum entry.
    #[must_use]
    pub fn peek(&self) -> Option<(AddressableHandle, &K, &V)> {
        self.inner.peek()
    }

    /// Removes and returns a minimum key-value pair.
    pub fn pop(&mut self) -> Option<(K, V)> {
        self.inner.pop(2)
    }

    /// Returns the key associated with `handle`.
    pub fn key(&self, handle: AddressableHandle) -> Result<&K, InvalidHandle> {
        self.inner.key(handle)
    }

    /// Returns the value associated with `handle`.
    pub fn value(&self, handle: AddressableHandle) -> Result<&V, InvalidHandle> {
        self.inner.value(handle)
    }

    /// Replaces the value associated with `handle`.
    pub fn set_value(&mut self, handle: AddressableHandle, value: V) -> Result<(), InvalidHandle> {
        self.inner.set_value(handle, value)
    }

    /// Decreases an entry's key and restores heap order.
    pub fn decrease_key(
        &mut self,
        handle: AddressableHandle,
        key: K,
    ) -> Result<(), DecreaseKeyError> {
        self.inner.decrease_key(handle, key, 2)
    }

    /// Removes the entry associated with `handle`.
    pub fn delete(&mut self, handle: AddressableHandle) -> Result<(K, V), InvalidHandle> {
        self.inner.delete(handle, 2)
    }

    /// Returns handles for all live entries in unspecified heap order.
    pub fn handles(&self) -> impl Iterator<Item = AddressableHandle> + '_ {
        self.inner.handles()
    }

    /// Returns the number of live entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns whether the heap contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    /// Removes all entries and invalidates every outstanding handle.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl<K, V, C> AddressableHeap<K, V> for BinaryArrayAddressableHeap<K, V, C>
where
    C: Comparator<K>,
{
    type Handle = AddressableHandle;

    fn push(&mut self, key: K, value: V) -> Self::Handle {
        Self::push(self, key, value)
    }

    fn peek(&self) -> Option<(Self::Handle, &K, &V)> {
        Self::peek(self)
    }

    fn pop(&mut self) -> Option<(K, V)> {
        Self::pop(self)
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

/// An array-backed d-ary min-heap with stable, checked entry handles.
///
/// The degree must be at least two. Larger degrees reduce insertion height but
/// increase comparisons during removal.
pub struct DaryArrayAddressableHeap<K, V, C = NaturalOrder> {
    inner: AddressableCore<K, V, C>,
    degree: usize,
}

impl<K: Ord, V> DaryArrayAddressableHeap<K, V> {
    /// Creates an empty heap with `degree` children per node.
    pub fn new(degree: usize) -> Result<Self, InvalidDegree> {
        Self::with_capacity(degree, DEFAULT_HEAP_CAPACITY)
    }

    /// Creates an empty heap with storage for at least `capacity` entries.
    pub fn with_capacity(degree: usize, capacity: usize) -> Result<Self, InvalidDegree> {
        Self::with_capacity_and_comparator(degree, capacity, NaturalOrder)
    }

    /// Builds a heap from key-value pairs in linear time.
    pub fn from_vec(degree: usize, entries: Vec<(K, V)>) -> Result<Self, InvalidDegree> {
        Self::from_vec_by(degree, entries, NaturalOrder)
    }
}

impl<K: Ord, V> Default for DaryArrayAddressableHeap<K, V> {
    fn default() -> Self {
        Self::new(2).expect("binary degree is valid")
    }
}

impl<K, V, C> DaryArrayAddressableHeap<K, V, C>
where
    C: Comparator<K>,
{
    /// Creates an empty heap ordered by `compare`.
    pub fn with_comparator(degree: usize, compare: C) -> Result<Self, InvalidDegree> {
        Self::with_capacity_and_comparator(degree, DEFAULT_HEAP_CAPACITY, compare)
    }

    /// Creates an empty heap with storage for at least `capacity` entries.
    pub fn with_capacity_and_comparator(
        degree: usize,
        capacity: usize,
        compare: C,
    ) -> Result<Self, InvalidDegree> {
        validate_degree(degree)?;
        Ok(Self {
            inner: AddressableCore::new(capacity, compare),
            degree,
        })
    }

    /// Builds a heap from key-value pairs in linear time using `compare`.
    pub fn from_vec_by(
        degree: usize,
        entries: Vec<(K, V)>,
        compare: C,
    ) -> Result<Self, InvalidDegree> {
        validate_degree(degree)?;
        Ok(Self {
            inner: AddressableCore::from_vec(entries, degree, compare),
            degree,
        })
    }

    /// Returns the number of children per node.
    #[must_use]
    pub const fn degree(&self) -> usize {
        self.degree
    }

    /// Returns the comparator used to order keys.
    #[must_use]
    pub fn comparator(&self) -> &C {
        self.inner.comparator()
    }

    /// Inserts an entry and returns a handle that addresses it while live.
    pub fn push(&mut self, key: K, value: V) -> AddressableHandle {
        self.inner.push(key, value, self.degree)
    }

    /// Alias for [`Self::push`], matching JHeaps terminology.
    pub fn insert(&mut self, key: K, value: V) -> AddressableHandle {
        self.push(key, value)
    }

    /// Returns the handle, key, and value of a minimum entry.
    #[must_use]
    pub fn peek(&self) -> Option<(AddressableHandle, &K, &V)> {
        self.inner.peek()
    }

    /// Removes and returns a minimum key-value pair.
    pub fn pop(&mut self) -> Option<(K, V)> {
        self.inner.pop(self.degree)
    }

    /// Returns the key associated with `handle`.
    pub fn key(&self, handle: AddressableHandle) -> Result<&K, InvalidHandle> {
        self.inner.key(handle)
    }

    /// Returns the value associated with `handle`.
    pub fn value(&self, handle: AddressableHandle) -> Result<&V, InvalidHandle> {
        self.inner.value(handle)
    }

    /// Replaces the value associated with `handle`.
    pub fn set_value(&mut self, handle: AddressableHandle, value: V) -> Result<(), InvalidHandle> {
        self.inner.set_value(handle, value)
    }

    /// Decreases an entry's key and restores heap order.
    pub fn decrease_key(
        &mut self,
        handle: AddressableHandle,
        key: K,
    ) -> Result<(), DecreaseKeyError> {
        self.inner.decrease_key(handle, key, self.degree)
    }

    /// Removes the entry associated with `handle`.
    pub fn delete(&mut self, handle: AddressableHandle) -> Result<(K, V), InvalidHandle> {
        self.inner.delete(handle, self.degree)
    }

    /// Returns handles for all live entries in unspecified heap order.
    pub fn handles(&self) -> impl Iterator<Item = AddressableHandle> + '_ {
        self.inner.handles()
    }

    /// Returns the number of live entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns whether the heap contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    /// Removes all entries and invalidates every outstanding handle.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl<K, V, C> AddressableHeap<K, V> for DaryArrayAddressableHeap<K, V, C>
where
    C: Comparator<K>,
{
    type Handle = AddressableHandle;

    fn push(&mut self, key: K, value: V) -> Self::Handle {
        Self::push(self, key, value)
    }

    fn peek(&self) -> Option<(Self::Handle, &K, &V)> {
        Self::peek(self)
    }

    fn pop(&mut self) -> Option<(K, V)> {
        Self::pop(self)
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

fn validate_degree(degree: usize) -> Result<(), InvalidDegree> {
    if degree < 2 {
        Err(InvalidDegree(degree))
    } else {
        Ok(())
    }
}
