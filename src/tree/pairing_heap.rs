use core::cmp::Ordering;

use crate::array::{DecreaseKeyError, InvalidHandle};
use crate::{AddressableHeap, MeldableAddressableHeap};

use super::core::{MeldError, NodeRef, TreeCore, TreeHandle};

/// An addressable pairing heap.
///
/// Insert, minimum removal, key decrease, and melding are amortized
/// `O(log n)`; looking up the minimum is `O(1)`. This is the classic two-pass
/// pairing-heap algorithm, not an array-backed priority queue.
pub struct PairingHeap<K, V = ()> {
    core: TreeCore<K, V>,
}

impl<K: Ord, V> Default for PairingHeap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> PairingHeap<K, V> {
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

    /// Returns mutable access to the value associated with `handle`.
    pub fn value_mut(&mut self, handle: TreeHandle) -> Result<&mut V, InvalidHandle> {
        self.core.value_mut(handle)
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

    /// Removes the entry associated with `handle`.
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
        self.core.node_mut(node).parent = None;
        self.core.node_mut(node).position = 0;
    }

    fn combine(&mut self, children: Vec<NodeRef>) -> Option<NodeRef> {
        let mut pairs = Vec::with_capacity(children.len().div_ceil(2));
        let mut iterator = children.into_iter();
        while let Some(first) = iterator.next() {
            pairs.push(self.link(Some(first), iterator.next()));
        }

        let mut root = None;
        for pair in pairs.into_iter().rev() {
            root = self.link(root, pair);
        }
        root
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

impl<K: Ord> PairingHeap<K, ()> {
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

impl<K: Ord, V> PairingHeap<K, V> {
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

impl<K: Ord, V> AddressableHeap<K, V> for PairingHeap<K, V> {
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

impl<K: Ord, V> MeldableAddressableHeap<K, V> for PairingHeap<K, V> {
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        Self::meld(self, other)
    }
}

crate::impl_heap_via_addressable!(PairingHeap);
crate::impl_meldable_heap_via_addressable!(PairingHeap);
