use core::cmp::Ordering;

use crate::array::{Comparator, DecreaseKeyError, InvalidHandle, NaturalOrder};
use crate::{AddressableHeap, Heap, MeldableAddressableHeap, MeldableHeap};

use super::core::{MeldError, NodeRef, TreeCore, TreeHandle};

/// An addressable leftist heap.
///
/// Insert, minimum removal, deletion, key decrease, and melding take
/// `O(log n)` worst-case time; finding the minimum is `O(1)`.
pub struct LeftistHeap<K, V = (), C = NaturalOrder> {
    core: TreeCore<K, V, C>,
}

impl<K: Ord, V> LeftistHeap<K, V> {
    /// Creates an empty heap using the natural ordering of keys.
    #[must_use]
    pub fn new() -> Self {
        Self::with_comparator(NaturalOrder)
    }
}

impl<K: Ord, V> Default for LeftistHeap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, C> LeftistHeap<K, V, C>
where
    C: Comparator<K>,
{
    /// Creates an empty heap ordered by `compare`.
    #[must_use]
    pub fn with_comparator(compare: C) -> Self {
        Self {
            core: TreeCore::new(compare),
        }
    }

    /// Returns the comparator used to order keys.
    #[must_use]
    pub fn comparator(&self) -> &C {
        self.core.comparator()
    }

    /// Inserts an entry and returns its checked handle.
    pub fn insert(&mut self, key: K, value: V) -> TreeHandle {
        self.try_insert(key, value)
            .expect("a meld donor cannot accept new entries")
    }

    /// Inserts an entry unless this heap was consumed as a meld donor.
    pub fn try_insert(&mut self, key: K, value: V) -> Result<TreeHandle, MeldError> {
        if !self.core.active {
            return Err(MeldError::ReceiverConsumed);
        }
        let node = self.core.insert_node(key, value);
        self.core.root = self.union(self.core.root, Some(node));
        self.core.len += 1;
        Ok(self.core.handle(node))
    }

    /// Returns the handle, key, and value of a minimum entry.
    #[must_use]
    pub fn peek_entry(&self) -> Option<(TreeHandle, &K, &V)> {
        if !self.core.active {
            return None;
        }
        self.core.root.map(|root| {
            let handle = self.core.handle(root);
            let node = self.core.node(root);
            (handle, &node.key, &node.value)
        })
    }

    /// Removes and returns a minimum entry.
    pub fn pop_entry(&mut self) -> Option<(K, V)> {
        if !self.core.active {
            return None;
        }
        let root = self.core.root?;
        let left = self.core.take_child(root, 0);
        let right = self.core.take_child(root, 1);
        self.core.root = self.union(left, right);
        self.core.len -= 1;
        let node = self.core.remove_node(root);
        Some((node.key, node.value))
    }

    /// Returns the key associated with `handle`.
    pub fn key(&self, handle: TreeHandle) -> Result<&K, InvalidHandle> {
        self.core.key(handle)
    }

    /// Returns the value associated with `handle`.
    pub fn value(&self, handle: TreeHandle) -> Result<&V, InvalidHandle> {
        self.core.value(handle)
    }

    /// Replaces the value associated with `handle`.
    pub fn set_value(&mut self, handle: TreeHandle, value: V) -> Result<(), InvalidHandle> {
        self.core.set_value(handle, value)
    }

    /// Decreases an entry's key and restores heap order.
    pub fn decrease_key(&mut self, handle: TreeHandle, key: K) -> Result<(), DecreaseKeyError> {
        let (node, order) = self.core.set_key(handle, key)?;
        if order == Ordering::Equal || self.core.root == Some(node) {
            return Ok(());
        }
        self.detach_and_reinsert(node);
        Ok(())
    }

    /// Removes the entry associated with `handle`.
    pub fn delete(&mut self, handle: TreeHandle) -> Result<(K, V), InvalidHandle> {
        let node = self.core.validate(handle)?;
        if self.core.root == Some(node) {
            return self.pop_entry().ok_or(InvalidHandle::Stale);
        }
        self.detach_node(node);
        self.core.len -= 1;
        let node = self.core.remove_node(node);
        Ok((node.key, node.value))
    }

    /// Returns the number of live entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.core.len
    }

    /// Returns whether the heap has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.core.len == 0
    }

    /// Removes every entry and invalidates outstanding handles.
    pub fn clear(&mut self) {
        if self.core.active {
            self.core.clear();
        }
    }

    fn detach_and_reinsert(&mut self, node: NodeRef) {
        self.detach_node(node);
        self.core.root = self.union(self.core.root, Some(node));
    }

    fn detach_node(&mut self, node: NodeRef) {
        let parent = self
            .core
            .parent(node)
            .expect("non-root tree node must have a parent");
        let position = self.core.position(node);
        self.core.set_child(parent, position, None);
        let left = self.core.take_child(node, 0);
        let right = self.core.take_child(node, 1);
        let replacement = self.union(left, right);
        self.core.set_child(parent, position, replacement);
        self.rebalance_from(Some(parent));
    }

    fn rebalance_from(&mut self, mut node: Option<NodeRef>) {
        while let Some(current) = node {
            let left = self.core.child(current, 0);
            let right = self.core.child(current, 1);
            let left_rank = left.map_or(0, |child| self.core.rank(child));
            let right_rank = right.map_or(0, |child| self.core.rank(child));
            if left_rank < right_rank {
                self.core.set_child(current, 0, right);
                self.core.set_child(current, 1, left);
            }
            let right_rank = self
                .core
                .child(current, 1)
                .map_or(0, |child| self.core.rank(child));
            self.core.set_rank(current, right_rank + 1);
            node = self.core.parent(current);
        }
    }

    fn union(&mut self, first: Option<NodeRef>, second: Option<NodeRef>) -> Option<NodeRef> {
        let (first, second) = match (first, second) {
            (None, other) | (other, None) => return other,
            (Some(first), Some(second)) => (first, second),
        };
        let (root, other) = if self.core.compare_nodes(first, second) == Ordering::Greater {
            (second, first)
        } else {
            (first, second)
        };
        let right = self.core.take_child(root, 1);
        let merged = self.union(right, Some(other));
        self.core.set_child(root, 1, merged);

        let left = self.core.child(root, 0);
        let right = self.core.child(root, 1);
        let left_rank = left.map_or(0, |child| self.core.rank(child));
        let right_rank = right.map_or(0, |child| self.core.rank(child));
        if left_rank < right_rank {
            self.core.set_child(root, 0, right);
            self.core.set_child(root, 1, left);
        }
        let right_rank = self
            .core
            .child(root, 1)
            .map_or(0, |child| self.core.rank(child));
        self.core.set_rank(root, right_rank + 1);
        Some(root)
    }
}

impl<K, C> LeftistHeap<K, (), C>
where
    C: Comparator<K>,
{
    /// Inserts a key into this value-less heap.
    pub fn push(&mut self, key: K) {
        self.try_insert(key, ())
            .expect("a meld donor cannot accept new entries");
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

impl<K, V, C> LeftistHeap<K, V, C>
where
    C: Comparator<K> + PartialEq,
{
    /// Melds `other` into this heap, consuming the donor on success.
    pub fn meld(&mut self, other: &mut Self) -> Result<(), MeldError> {
        if !self.core.active {
            return Err(MeldError::ReceiverConsumed);
        }
        if !other.core.active {
            return Err(MeldError::DonorConsumed);
        }
        if self.core.compare != other.core.compare {
            return Err(MeldError::IncompatibleComparator);
        }
        let other_root = other.core.root;
        self.core.take_arenas_from(&mut other.core);
        self.core.root = self.union(self.core.root, other_root);
        self.core.len += other.core.len;
        other.core.root = None;
        other.core.len = 0;
        other.core.active = false;
        Ok(())
    }
}

impl<K, V, C> AddressableHeap<K, V> for LeftistHeap<K, V, C>
where
    C: Comparator<K>,
{
    type Handle = TreeHandle;

    fn push(&mut self, key: K, value: V) -> Self::Handle {
        self.try_insert(key, value)
            .expect("a meld donor cannot accept new entries")
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

impl<T, C> Heap<T> for LeftistHeap<T, (), C>
where
    C: Comparator<T>,
{
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

impl<K, V, C> MeldableAddressableHeap<K, V> for LeftistHeap<K, V, C>
where
    C: Comparator<K> + PartialEq,
{
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        Self::meld(self, other)
    }
}

impl<T, C> MeldableHeap<T> for LeftistHeap<T, (), C>
where
    C: Comparator<T> + PartialEq,
{
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        Self::meld(self, other)
    }
}
