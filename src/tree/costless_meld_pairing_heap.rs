use core::cmp::Ordering;

use crate::array::{DecreaseKeyError, InvalidHandle};
use crate::{AddressableHeap, Heap, MeldableAddressableHeap, MeldableHeap};

use super::core::{MeldError, NodeRef, TreeCore, TreeHandle};

/// An addressable costless-meld pairing heap.
///
/// Decreased subtrees are held in a separate decrease pool instead of being
/// immediately linked into the main pairing tree. The pool has a cached
/// minimum and is periodically consolidated at a logarithmic-size threshold,
/// retaining the deferred-work design of costless-meld pairing heaps.
pub struct CostlessMeldPairingHeap<K, V = ()> {
    core: TreeCore<K, V>,
    decrease_pool: Vec<NodeRef>,
    pool_minimum: Option<NodeRef>,
}

impl<K: Ord, V> Default for CostlessMeldPairingHeap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> CostlessMeldPairingHeap<K, V> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: TreeCore::new(),
            decrease_pool: Vec::new(),
            pool_minimum: None,
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
        let root = self.minimum_node()?;
        let handle = self.core.handle(root);
        let node = self.core.node(root);
        Some((handle, &node.key, &node.value))
    }

    /// Removes and returns a minimum entry.
    pub fn pop_entry(&mut self) -> Option<(K, V)> {
        if !self.core.active || self.core.len == 0 {
            return None;
        }
        self.consolidate_pool();
        let root = self.core.root.expect("a nonempty heap has a main root");
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

    /// Decreases an entry's key, deferring reassembly in the decrease pool.
    pub fn decrease_key(&mut self, handle: TreeHandle, key: K) -> Result<(), DecreaseKeyError> {
        let (node, order) = self.core.set_key(handle, key)?;
        if order == Ordering::Equal {
            return Ok(());
        }
        if self.decrease_pool.contains(&node) {
            self.refresh_pool_minimum();
            return Ok(());
        }
        if self.core.root == Some(node) {
            return Ok(());
        }
        let parent = self
            .core
            .parent(node)
            .expect("a non-root pairing node has a parent");
        if self.core.compare_nodes(node, parent) == Ordering::Less {
            self.cut_from_parent(node);
            self.add_to_pool(node);
        }
        Ok(())
    }

    /// Removes and returns the entry associated with `handle`.
    pub fn delete(&mut self, handle: TreeHandle) -> Result<(K, V), InvalidHandle> {
        let node = self.core.validate(handle)?;
        if let Some(index) = self.decrease_pool.iter().position(|&entry| entry == node) {
            self.decrease_pool.swap_remove(index);
            let children = self.core.take_children(node);
            let replacement = self.combine(children);
            self.core.root = self.link(self.core.root, replacement);
            self.core.len -= 1;
            self.refresh_pool_minimum();
            let node = self.core.remove_node(node);
            return Ok((node.key, node.value));
        }

        self.consolidate_pool();
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
            self.decrease_pool.clear();
            self.pool_minimum = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn assert_invariants(&self) {
        let roots = self
            .core
            .root
            .into_iter()
            .chain(self.decrease_pool.iter().copied());
        self.core.assert_heap_forest(roots);
        assert_eq!(self.pool_minimum.is_some(), !self.decrease_pool.is_empty());
        if let Some(pool_minimum) = self.pool_minimum {
            assert!(self.decrease_pool.contains(&pool_minimum));
        }
    }

    fn minimum_node(&self) -> Option<NodeRef> {
        match (self.core.root, self.pool_minimum) {
            (None, other) | (other, None) => other,
            (Some(root), Some(pool)) => {
                if self.core.compare_nodes(pool, root) == Ordering::Less {
                    Some(pool)
                } else {
                    Some(root)
                }
            }
        }
    }

    fn add_to_pool(&mut self, node: NodeRef) {
        self.decrease_pool.push(node);
        if self
            .pool_minimum
            .is_none_or(|minimum| self.core.compare_nodes(node, minimum) == Ordering::Less)
        {
            self.pool_minimum = Some(node);
        }
        if self.decrease_pool.len() >= self.pool_limit() {
            self.consolidate_pool();
        }
    }

    fn pool_limit(&self) -> usize {
        self.core.len.max(2).ilog2() as usize + 2
    }

    fn refresh_pool_minimum(&mut self) {
        self.pool_minimum = self
            .decrease_pool
            .iter()
            .copied()
            .min_by(|left, right| self.core.compare_nodes(*left, *right));
    }

    fn consolidate_pool(&mut self) {
        if self.decrease_pool.is_empty() {
            return;
        }
        let pool = core::mem::take(&mut self.decrease_pool);
        self.pool_minimum = None;
        let pooled_tree = self.combine(pool);
        self.core.root = self.link(self.core.root, pooled_tree);
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
        let mut pairs = Vec::with_capacity(children.len().div_ceil(2));
        let mut children = children.into_iter();
        while let Some(first) = children.next() {
            pairs.push(self.link(Some(first), children.next()));
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

impl<K: Ord> CostlessMeldPairingHeap<K, ()> {
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

impl<K: Ord, V> CostlessMeldPairingHeap<K, V> {
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
        self.decrease_pool.append(&mut other.decrease_pool);
        self.core.len += other.core.len;
        other.core.root = None;
        other.core.len = 0;
        other.core.active = false;
        other.pool_minimum = None;
        self.refresh_pool_minimum();
        Ok(())
    }
}

impl<K: Ord, V> AddressableHeap<K, V> for CostlessMeldPairingHeap<K, V> {
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

    fn value_mut(&mut self, handle: Self::Handle) -> Result<&mut V, InvalidHandle> {
        self.value_mut(handle)
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

impl<T: Ord> Heap<T> for CostlessMeldPairingHeap<T, ()> {
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

impl<K: Ord, V> MeldableAddressableHeap<K, V> for CostlessMeldPairingHeap<K, V> {
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        self.meld(other)
    }
}

impl<T: Ord> MeldableHeap<T> for CostlessMeldPairingHeap<T, ()> {
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        self.meld(other)
    }
}
