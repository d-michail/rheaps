use core::cmp::Ordering;

use crate::array::{DecreaseKeyError, InvalidHandle};
use crate::{AddressableHeap, Heap, MeldableAddressableHeap, MeldableHeap};

use super::core::{MeldError, NodeRef, TreeCore, TreeHandle};

/// An addressable Fibonacci heap.
///
/// Insertions, melds, and key decreases are amortized `O(1)`. Removing a
/// minimum or deleting an entry is amortized `O(log n)`. Nodes are kept in a
/// forest of heap-ordered trees; removal consolidates roots of equal degree.
pub struct FibonacciHeap<K, V = ()> {
    core: TreeCore<K, V>,
    roots: Vec<NodeRef>,
}

impl<K: Ord, V> Default for FibonacciHeap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> FibonacciHeap<K, V> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: TreeCore::new(),
            roots: Vec::new(),
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
        self.add_root(node);
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
        self.core.root.and_then(|root| self.remove_root(root))
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

    /// Decreases an entry's key and restores heap order using cascading cuts.
    pub fn decrease_key(&mut self, handle: TreeHandle, key: K) -> Result<(), DecreaseKeyError> {
        let (node, order) = self.core.set_key(handle, key)?;
        if order == Ordering::Equal {
            return Ok(());
        }
        if let Some(parent) = self.core.parent(node)
            && self.core.compare_nodes(node, parent) == Ordering::Less
        {
            self.cut(node, parent);
            self.cascading_cut(parent);
        }
        if self
            .core
            .root
            .is_none_or(|minimum| self.core.compare_nodes(node, minimum) == Ordering::Less)
        {
            self.core.root = Some(node);
        }
        Ok(())
    }

    /// Removes and returns the entry associated with `handle`.
    pub fn delete(&mut self, handle: TreeHandle) -> Result<(K, V), InvalidHandle> {
        let node = self.core.validate(handle)?;
        if let Some(parent) = self.core.parent(node) {
            self.cut(node, parent);
            self.cascading_cut(parent);
        }
        self.remove_root(node).ok_or(InvalidHandle::Stale)
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
            self.roots.clear();
        }
    }

    #[cfg(test)]
    pub(crate) fn assert_invariants(&self) {
        self.core.assert_heap_forest(self.roots.iter().copied());
        assert_eq!(self.core.root.is_none(), self.roots.is_empty());
        for root in &self.roots {
            assert_eq!(self.core.node(*root).parent, None);
            assert_eq!(self.core.rank(*root), self.core.node(*root).children.len());
        }
    }

    fn add_root(&mut self, node: NodeRef) {
        {
            let entry = self.core.node_mut(node);
            entry.parent = None;
            entry.position = 0;
            entry.marked = false;
        }
        self.roots.push(node);
        if self
            .core
            .root
            .is_none_or(|minimum| self.core.compare_nodes(node, minimum) == Ordering::Less)
        {
            self.core.root = Some(node);
        }
    }

    fn remove_root(&mut self, root: NodeRef) -> Option<(K, V)> {
        let index = self.roots.iter().position(|&node| node == root)?;
        self.roots.swap_remove(index);
        for child in self.core.take_children(root) {
            self.add_root(child);
        }
        self.core.len -= 1;
        let node = self.core.remove_node(root);
        self.consolidate();
        Some((node.key, node.value))
    }

    fn cut(&mut self, node: NodeRef, parent: NodeRef) {
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
        let degree = self.core.node(parent).children.len();
        self.core.set_rank(parent, degree);
        self.add_root(node);
    }

    fn cascading_cut(&mut self, mut node: NodeRef) {
        while let Some(parent) = self.core.parent(node) {
            if !self.core.marked(node) {
                self.core.set_marked(node, true);
                break;
            }
            self.cut(node, parent);
            node = parent;
        }
    }

    fn consolidate(&mut self) {
        self.core.root = None;
        let roots = core::mem::take(&mut self.roots);
        let mut buckets = Vec::<Option<NodeRef>>::new();
        for node in roots {
            {
                let entry = self.core.node_mut(node);
                entry.parent = None;
                entry.position = 0;
                entry.marked = false;
            }
            let mut tree = node;
            loop {
                let rank = self.core.rank(tree);
                if buckets.len() <= rank {
                    buckets.resize(rank + 1, None);
                }
                if let Some(other) = buckets[rank].take() {
                    tree = self.link(other, tree);
                } else {
                    buckets[rank] = Some(tree);
                    break;
                }
            }
        }
        self.roots = buckets.into_iter().flatten().collect();
        self.refresh_minimum();
    }

    fn link(&mut self, first: NodeRef, second: NodeRef) -> NodeRef {
        let (parent, child) = if self.core.compare_nodes(first, second) == Ordering::Greater {
            (second, first)
        } else {
            (first, second)
        };
        self.core.push_child(parent, child);
        self.core.set_marked(child, false);
        self.core.set_rank(parent, self.core.rank(parent) + 1);
        parent
    }

    fn refresh_minimum(&mut self) {
        self.core.root = self
            .roots
            .iter()
            .copied()
            .min_by(|left, right| self.core.compare_nodes(*left, *right));
    }
}

impl<K: Ord> FibonacciHeap<K, ()> {
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

impl<K: Ord, V> FibonacciHeap<K, V> {
    /// Melds `other` into this heap, consuming the donor on success.
    pub fn meld(&mut self, other: &mut Self) -> Result<(), MeldError> {
        if !self.core.active {
            return Err(MeldError::ReceiverConsumed);
        }
        if !other.core.active {
            return Err(MeldError::DonorConsumed);
        }
        self.core.take_arenas_from(&mut other.core);
        self.roots.append(&mut other.roots);
        self.core.len += other.core.len;
        other.core.root = None;
        other.core.len = 0;
        other.core.active = false;
        self.refresh_minimum();
        Ok(())
    }
}

impl<K: Ord, V> AddressableHeap<K, V> for FibonacciHeap<K, V> {
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

impl<T: Ord> Heap<T> for FibonacciHeap<T, ()> {
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

impl<K: Ord, V> MeldableAddressableHeap<K, V> for FibonacciHeap<K, V> {
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        self.meld(other)
    }
}

impl<T: Ord> MeldableHeap<T> for FibonacciHeap<T, ()> {
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        self.meld(other)
    }
}
