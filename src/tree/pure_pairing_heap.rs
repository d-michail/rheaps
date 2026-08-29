use core::cmp::Ordering;

use crate::array::{DecreaseKeyError, InvalidHandle};
use crate::{AddressableHeap, Heap, MeldableAddressableHeap, MeldableHeap};

use super::core::{MeldError, NodeRef, TreeCore, TreeHandle};

/// An addressable pure pairing heap.
///
/// This variant uses the Tarjan--Xu single pairing pass: after pairing the
/// children of a deleted root, the least winner becomes the new root and all
/// remaining winners become its children. It avoids the assembly pass used by
/// [`super::PairingHeap`].
pub struct PurePairingHeap<K, V = ()> {
    core: TreeCore<K, V>,
}

impl<K: Ord, V> Default for PurePairingHeap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> PurePairingHeap<K, V> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: TreeCore::new(),
        }
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
        self.core.root = self.link(self.core.root, Some(node));
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
        let children = self.core.take_children(root);
        self.core.root = self.combine(children);
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
        self.cut_from_parent(node);
        self.core.root = self.link(self.core.root, Some(node));
        Ok(())
    }

    /// Removes and returns the entry associated with `handle`.
    pub fn delete(&mut self, handle: TreeHandle) -> Result<(K, V), InvalidHandle> {
        let node = self.core.validate(handle)?;
        if self.core.root == Some(node) {
            return self.pop_entry().ok_or(InvalidHandle::Stale);
        }
        self.cut_from_parent(node);
        let children = self.core.take_children(node);
        let replacement = self.combine(children);
        self.core.root = self.link(self.core.root, replacement);
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

    #[cfg(test)]
    pub(crate) fn assert_invariants(&self) {
        self.core.assert_heap_forest(self.core.root);
    }

    fn cut_from_parent(&mut self, node: NodeRef) {
        let parent = self
            .core
            .parent(node)
            .expect("non-root tree node must have a parent");
        let position = self.core.position(node);
        let moved = {
            let children = &mut self.core.node_mut(parent).children;
            let removed = children.swap_remove(position);
            debug_assert_eq!(removed, Some(node));
            children.get(position).copied().flatten()
        };
        if let Some(moved) = moved {
            self.core.node_mut(moved).position = position;
        }
        let entry = self.core.node_mut(node);
        entry.parent = None;
        entry.position = 0;
    }

    fn combine(&mut self, children: Vec<NodeRef>) -> Option<NodeRef> {
        let mut winners = Vec::with_capacity(children.len().div_ceil(2));
        let mut children = children.into_iter();
        while let Some(first) = children.next() {
            winners.push(
                self.link(Some(first), children.next())
                    .expect("pair has a root"),
            );
        }
        let minimum = winners
            .iter()
            .copied()
            .min_by(|left, right| self.core.compare_nodes(*left, *right))?;
        for winner in winners {
            if winner != minimum {
                self.core.push_child(minimum, winner);
            }
        }
        Some(minimum)
    }

    fn link(&mut self, first: Option<NodeRef>, second: Option<NodeRef>) -> Option<NodeRef> {
        let (first, second) = match (first, second) {
            (None, other) | (other, None) => return other,
            (Some(first), Some(second)) => (first, second),
        };
        let (root, child) = if self.core.compare_nodes(first, second) == Ordering::Greater {
            (second, first)
        } else {
            (first, second)
        };
        self.core.push_child(root, child);
        Some(root)
    }
}

impl<K: Ord> PurePairingHeap<K, ()> {
    /// Inserts a key into this value-less heap.
    pub fn push(&mut self, key: K) {
        self.insert(key, ());
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

impl<K: Ord, V> PurePairingHeap<K, V> {
    /// Melds `other` into this heap, consuming the donor on success.
    pub fn meld(&mut self, other: &mut Self) -> Result<(), MeldError> {
        if !self.core.active {
            return Err(MeldError::ReceiverConsumed);
        }
        if !other.core.active {
            return Err(MeldError::DonorConsumed);
        }
        let other_root = other.core.root;
        self.core.take_arenas_from(&mut other.core);
        self.core.root = self.link(self.core.root, other_root);
        self.core.len += other.core.len;
        other.core.root = None;
        other.core.len = 0;
        other.core.active = false;
        Ok(())
    }
}

impl<K: Ord, V> AddressableHeap<K, V> for PurePairingHeap<K, V> {
    type Handle = TreeHandle;

    fn push(&mut self, key: K, value: V) -> Self::Handle {
        self.insert(key, value)
    }

    fn peek(&self) -> Option<(Self::Handle, &K, &V)> {
        self.peek_entry()
    }

    fn pop(&mut self) -> Option<(K, V)> {
        self.pop_entry()
    }

    fn key(&self, handle: Self::Handle) -> Result<&K, InvalidHandle> {
        self.key(handle)
    }

    fn value(&self, handle: Self::Handle) -> Result<&V, InvalidHandle> {
        self.value(handle)
    }

    fn set_value(&mut self, handle: Self::Handle, value: V) -> Result<(), InvalidHandle> {
        self.set_value(handle, value)
    }

    fn decrease_key(&mut self, handle: Self::Handle, key: K) -> Result<(), DecreaseKeyError> {
        self.decrease_key(handle, key)
    }

    fn delete(&mut self, handle: Self::Handle) -> Result<(K, V), InvalidHandle> {
        self.delete(handle)
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn clear(&mut self) {
        self.clear();
    }
}

impl<T: Ord> Heap<T> for PurePairingHeap<T, ()> {
    fn push(&mut self, value: T) {
        self.push(value);
    }

    fn peek(&self) -> Option<&T> {
        self.peek()
    }

    fn pop(&mut self) -> Option<T> {
        self.pop()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn clear(&mut self) {
        self.clear();
    }
}

impl<K: Ord, V> MeldableAddressableHeap<K, V> for PurePairingHeap<K, V> {
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        self.meld(other)
    }
}

impl<T: Ord> MeldableHeap<T> for PurePairingHeap<T, ()> {
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        self.meld(other)
    }
}
