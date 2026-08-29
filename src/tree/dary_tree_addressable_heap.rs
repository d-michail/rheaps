use core::cmp::Ordering;
use core::fmt;

use crate::AddressableHeap;
use crate::array::{DecreaseKeyError, InvalidHandle};

use super::core::{TreeHandle, next_domain_id};

struct Entry<K, V> {
    key: K,
    value: V,
    position: usize,
}

struct EntrySlot<K, V> {
    entry: Option<Entry<K, V>>,
    generation: u64,
}

struct Position {
    parent: Option<usize>,
    children: Vec<Option<usize>>,
    entry: usize,
}

/// The invalid branching factor supplied to [`DaryTreeAddressableHeap`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidBranchingFactor(pub usize);

impl fmt::Display for InvalidBranchingFactor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "branching factor {} must be a power of two and at least two",
            self.0
        )
    }
}

impl std::error::Error for InvalidBranchingFactor {}

/// An explicit power-of-two d-ary tree addressable heap.
///
/// Entries reside in nodes of a complete d-ary tree. Entry handles remain
/// stable while entries move between tree nodes to restore heap order.
pub struct DaryTreeAddressableHeap<K, V = ()> {
    degree: usize,
    entries: Vec<EntrySlot<K, V>>,
    free_entries: Vec<usize>,
    positions: Vec<Position>,
    domain: u64,
}

impl<K: Ord, V> DaryTreeAddressableHeap<K, V> {
    /// Creates an empty heap with a power-of-two branching factor.
    pub fn new(degree: usize) -> Result<Self, InvalidBranchingFactor> {
        if degree < 2 || !degree.is_power_of_two() {
            return Err(InvalidBranchingFactor(degree));
        }
        Ok(Self {
            degree,
            entries: Vec::new(),
            free_entries: Vec::new(),
            positions: Vec::new(),
            domain: next_domain_id(),
        })
    }

    /// Returns the number of children per tree node.
    #[must_use]
    pub const fn degree(&self) -> usize {
        self.degree
    }

    /// Inserts an entry and returns a checked handle.
    pub fn insert(&mut self, key: K, value: V) -> TreeHandle {
        let position = self.positions.len();
        let entry = self.insert_entry(key, value, position);
        if position == 0 {
            self.positions.push(Position {
                parent: None,
                children: (0..self.degree).map(|_| None).collect(),
                entry,
            });
        } else {
            let parent = (position - 1) / self.degree;
            let side = (position - 1) % self.degree;
            self.positions.push(Position {
                parent: Some(parent),
                children: (0..self.degree).map(|_| None).collect(),
                entry,
            });
            self.positions[parent].children[side] = Some(position);
            self.fix_up(position);
        }
        self.handle(entry)
    }

    /// Returns the handle, key, and value of a minimum entry.
    #[must_use]
    pub fn peek_entry(&self) -> Option<(TreeHandle, &K, &V)> {
        let position = self.positions.first()?;
        let entry = self.entry(position.entry);
        Some((self.handle(position.entry), &entry.key, &entry.value))
    }

    /// Removes and returns a minimum entry.
    pub fn pop_entry(&mut self) -> Option<(K, V)> {
        let len = self.positions.len();
        if len == 0 {
            return None;
        }
        let minimum = self.positions[0].entry;
        if len == 1 {
            self.positions.pop();
            return Some(self.remove_entry(minimum).into_pair());
        }
        let last = len - 1;
        self.swap_entries(0, last);
        self.detach_last(last);
        let minimum = self.remove_entry(minimum);
        self.fix_down(0);
        Some(minimum.into_pair())
    }

    /// Returns the key identified by `handle`.
    pub fn key(&self, handle: TreeHandle) -> Result<&K, InvalidHandle> {
        Ok(&self.entry(self.validate(handle)?).key)
    }

    /// Returns the value identified by `handle`.
    pub fn value(&self, handle: TreeHandle) -> Result<&V, InvalidHandle> {
        Ok(&self.entry(self.validate(handle)?).value)
    }

    /// Returns mutable access to the value identified by `handle`.
    pub fn value_mut(&mut self, handle: TreeHandle) -> Result<&mut V, InvalidHandle> {
        let entry = self.validate(handle)?;
        Ok(&mut self.entry_mut(entry).value)
    }

    /// Decreases an entry's key and restores heap order.
    pub fn decrease_key(&mut self, handle: TreeHandle, key: K) -> Result<(), DecreaseKeyError> {
        let entry = self
            .validate(handle)
            .map_err(DecreaseKeyError::InvalidHandle)?;
        if key > self.entry(entry).key {
            return Err(DecreaseKeyError::NotDecreased);
        }
        let position = self.entry(entry).position;
        self.entry_mut(entry).key = key;
        self.fix_up(position);
        Ok(())
    }

    /// Removes and returns the entry identified by `handle`.
    pub fn delete(&mut self, handle: TreeHandle) -> Result<(K, V), InvalidHandle> {
        let entry = self.validate(handle)?;
        let position = self.entry(entry).position;
        let last = self.positions.len() - 1;
        if position != last {
            self.swap_entries(position, last);
        }
        self.detach_last(last);
        let removed = self.remove_entry(entry);
        if position != last {
            self.restore_at(position);
        }
        Ok(removed.into_pair())
    }

    /// Returns the number of live entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Returns whether the heap contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Removes all entries and invalidates every outstanding handle.
    pub fn clear(&mut self) {
        self.positions.clear();
        self.free_entries.clear();
        for (index, slot) in self.entries.iter_mut().enumerate() {
            if slot.entry.take().is_some() {
                slot.generation = slot.generation.wrapping_add(1);
            }
            self.free_entries.push(index);
        }
    }

    fn validate(&self, handle: TreeHandle) -> Result<usize, InvalidHandle> {
        if handle.domain != self.domain {
            return Err(InvalidHandle::ForeignHeap);
        }
        let Some(slot) = self.entries.get(handle.slot) else {
            return Err(InvalidHandle::Stale);
        };
        if slot.generation != handle.generation || slot.entry.is_none() {
            return Err(InvalidHandle::Stale);
        }
        Ok(handle.slot)
    }

    fn insert_entry(&mut self, key: K, value: V, position: usize) -> usize {
        if let Some(slot) = self.free_entries.pop() {
            self.entries[slot].entry = Some(Entry {
                key,
                value,
                position,
            });
            slot
        } else {
            self.entries.push(EntrySlot {
                entry: Some(Entry {
                    key,
                    value,
                    position,
                }),
                generation: 0,
            });
            self.entries.len() - 1
        }
    }

    fn remove_entry(&mut self, entry: usize) -> Entry<K, V> {
        let slot = &mut self.entries[entry];
        let removed = slot.entry.take().expect("entry must be live");
        slot.generation = slot.generation.wrapping_add(1);
        self.free_entries.push(entry);
        removed
    }

    fn entry(&self, entry: usize) -> &Entry<K, V> {
        self.entries[entry]
            .entry
            .as_ref()
            .expect("entry must be live")
    }

    fn entry_mut(&mut self, entry: usize) -> &mut Entry<K, V> {
        self.entries[entry]
            .entry
            .as_mut()
            .expect("entry must be live")
    }

    fn handle(&self, entry: usize) -> TreeHandle {
        TreeHandle {
            domain: self.domain,
            slot: entry,
            generation: self.entries[entry].generation,
        }
    }

    fn swap_entries(&mut self, left: usize, right: usize) {
        let left_entry = self.positions[left].entry;
        let right_entry = self.positions[right].entry;
        self.positions[left].entry = right_entry;
        self.positions[right].entry = left_entry;
        self.entry_mut(left_entry).position = right;
        self.entry_mut(right_entry).position = left;
    }

    fn detach_last(&mut self, last: usize) {
        debug_assert_eq!(last + 1, self.positions.len());
        if let Some(parent) = self.positions[last].parent {
            let side = self.positions[parent]
                .children
                .iter()
                .position(|&child| child == Some(last))
                .expect("last node must belong to its parent");
            self.positions[parent].children[side] = None;
        }
        self.positions.pop();
    }

    fn restore_at(&mut self, position: usize) {
        if let Some(parent) = self.positions[position].parent
            && self.compare_positions(position, parent) == Ordering::Less
        {
            self.fix_up(position);
            return;
        }
        self.fix_down(position);
    }

    fn fix_up(&mut self, mut position: usize) {
        while let Some(parent) = self.positions[position].parent {
            if self.compare_positions(position, parent) != Ordering::Less {
                return;
            }
            self.swap_entries(position, parent);
            position = parent;
        }
    }

    fn fix_down(&mut self, mut position: usize) {
        loop {
            let child = self.positions[position]
                .children
                .iter()
                .flatten()
                .copied()
                .min_by(|left, right| self.compare_positions(*left, *right));
            let Some(child) = child else {
                return;
            };
            if self.compare_positions(position, child) != Ordering::Greater {
                return;
            }
            self.swap_entries(position, child);
            position = child;
        }
    }

    fn compare_positions(&self, left: usize, right: usize) -> Ordering {
        self.entry(self.positions[left].entry)
            .key
            .cmp(&self.entry(self.positions[right].entry).key)
    }
}

impl<K: Ord, V> AddressableHeap<K, V> for DaryTreeAddressableHeap<K, V> {
    type Handle = TreeHandle;

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

impl<K, V> Entry<K, V> {
    fn into_pair(self) -> (K, V) {
        (self.key, self.value)
    }
}

impl<K: Ord> DaryTreeAddressableHeap<K, ()> {
    /// Inserts a key into this value-less heap and returns a checked handle.
    pub fn push(&mut self, key: K) -> TreeHandle {
        self.insert(key, ())
    }

    /// Returns the minimum key, if present.
    #[must_use]
    pub fn peek(&self) -> Option<&K> {
        self.peek_entry().map(|(_, key, _)| key)
    }

    /// Removes and returns the minimum key, if present.
    pub fn pop(&mut self) -> Option<K> {
        self.pop_entry().map(|(key, ())| key)
    }
}

crate::impl_heap_via_addressable!(DaryTreeAddressableHeap);
