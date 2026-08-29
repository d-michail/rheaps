use core::cmp::Ordering;

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
    children: [Option<usize>; 2],
    entry: usize,
}

/// An explicit complete binary-tree addressable heap.
///
/// Insert, minimum removal, deletion, and key decreases are `O(log n)`;
/// looking up the minimum is `O(1)`. Nodes form an explicit binary tree while
/// entries are moved between nodes, preserving the identity of their handles.
pub struct BinaryTreeAddressableHeap<K, V = ()> {
    entries: Vec<EntrySlot<K, V>>,
    free_entries: Vec<usize>,
    positions: Vec<Position>,
    domain: u64,
}

impl<K: Ord, V> BinaryTreeAddressableHeap<K, V> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            free_entries: Vec::new(),
            positions: Vec::new(),
            domain: next_domain_id(),
        }
    }
}

impl<K: Ord, V> Default for BinaryTreeAddressableHeap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> BinaryTreeAddressableHeap<K, V> {
    /// Inserts an entry and returns a checked handle.
    pub fn push(&mut self, key: K, value: V) -> TreeHandle {
        let position = self.positions.len();
        let entry = self.insert_entry(key, value, position);
        if position == 0 {
            self.positions.push(Position {
                parent: None,
                children: [None, None],
                entry,
            });
        } else {
            let parent = self.node_at(position.div_ceil(2));
            let side = 1 - (position & 1);
            self.positions.push(Position {
                parent: Some(parent),
                children: [None, None],
                entry,
            });
            self.positions[parent].children[side] = Some(position);
            self.fix_up(position);
        }
        self.handle(entry)
    }

    /// Alias for [`Self::push`], matching JHeaps terminology.
    pub fn insert(&mut self, key: K, value: V) -> TreeHandle {
        self.push(key, value)
    }

    /// Returns the handle, key, and value of a minimum entry.
    #[must_use]
    pub fn peek(&self) -> Option<(TreeHandle, &K, &V)> {
        let position = self.positions.first()?;
        let entry = self.entry(position.entry);
        Some((self.handle(position.entry), &entry.key, &entry.value))
    }

    /// Removes and returns a minimum entry.
    pub fn pop(&mut self) -> Option<(K, V)> {
        let len = self.positions.len();
        if len == 0 {
            return None;
        }
        let minimum = self.positions[0].entry;
        if len == 1 {
            self.positions.pop();
            return Some(self.remove_entry(minimum).into_pair());
        }

        let last = self.node_at(len);
        self.swap_entries(0, last);
        self.detach_last(last);
        let minimum = self.remove_entry(minimum);
        self.fix_down(0);
        Some(minimum.into_pair())
    }

    /// Returns the key associated with `handle`.
    pub fn key(&self, handle: TreeHandle) -> Result<&K, InvalidHandle> {
        Ok(&self.entry(self.validate(handle)?).key)
    }

    /// Returns the value associated with `handle`.
    pub fn value(&self, handle: TreeHandle) -> Result<&V, InvalidHandle> {
        Ok(&self.entry(self.validate(handle)?).value)
    }

    /// Replaces the value associated with `handle`.
    pub fn set_value(&mut self, handle: TreeHandle, value: V) -> Result<(), InvalidHandle> {
        let entry = self.validate(handle)?;
        self.entry_mut(entry).value = value;
        Ok(())
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

    /// Removes the entry associated with `handle`.
    pub fn delete(&mut self, handle: TreeHandle) -> Result<(K, V), InvalidHandle> {
        let entry = self.validate(handle)?;
        let position = self.entry(entry).position;
        let len = self.positions.len();
        let last = self.node_at(len);
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

    /// Returns whether this heap has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Removes every entry and invalidates every outstanding handle.
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

    fn node_at(&self, number: usize) -> usize {
        debug_assert!((1..=self.positions.len()).contains(&number));
        let highest_bit = 1usize << (usize::BITS - number.leading_zeros() - 1);
        let mut node = 0;
        let mut bit = highest_bit >> 1;
        while bit != 0 {
            let side = usize::from(number & bit != 0);
            node = self.positions[node].children[side].expect("complete tree path must exist");
            bit >>= 1;
        }
        node
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
            let side = usize::from(self.positions[parent].children[1] == Some(last));
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
                break;
            }
            self.swap_entries(position, parent);
            position = parent;
        }
    }

    fn fix_down(&mut self, mut position: usize) {
        loop {
            let [left, right] = self.positions[position].children;
            let Some(mut child) = left else {
                return;
            };
            if let Some(right) = right
                && self.compare_positions(right, child) == Ordering::Less
            {
                child = right;
            }
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

impl<K: Ord, V> AddressableHeap<K, V> for BinaryTreeAddressableHeap<K, V> {
    type Handle = TreeHandle;

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

impl<K, V> Entry<K, V> {
    fn into_pair(self) -> (K, V) {
        (self.key, self.value)
    }
}
