use core::cmp::Ordering;
use core::convert::Infallible;

use crate::error::{DecreaseKeyError, InvalidHandle};
use crate::{AddressableHeap, DecreaseKeyHeap, MeldableAddressableHeap};

use super::core::{NodeRef, TreeCore, TreeHandle};

/// An addressable simple Fibonacci heap.
///
/// Unlike a classic Fibonacci heap, this implementation maintains one
/// heap-ordered root tree. Root removal promotes and rank-consolidates its
/// children; a decreased node is cut and linked directly with that root.
pub struct SimpleFibonacciHeap<K, V = ()> {
    core: TreeCore<K, V>,
}

impl<K: Ord, V> Default for SimpleFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> SimpleFibonacciHeap<K, V> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: TreeCore::new(),
        }
    }

    /// Inserts an entry and returns its checked handle.
    pub fn insert(&mut self, key: K, value: V) -> TreeHandle {
        let node = self.core.insert_node(key, value);
        self.core.root = self.link(self.core.root, Some(node));
        self.core.len += 1;
        self.core.handle(node)
    }

    /// Returns the handle, key, and value of a minimum entry.
    #[must_use]
    pub fn peek_entry(&self) -> Option<(TreeHandle, &K, &V)> {
        self.core.root.map(|root| {
            let handle = self.core.handle(root);
            let node = self.core.node(root);
            (handle, &node.key, &node.value)
        })
    }

    /// Removes and returns a minimum entry.
    pub fn pop_entry(&mut self) -> Option<(K, V)> {
        let root = self.core.root?;
        let children = self.core.take_children(root);
        self.core.root = self.consolidate(children);
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

    /// Decreases an entry's key and cuts it if its parent order is violated.
    pub fn decrease_key(&mut self, handle: TreeHandle, key: K) -> Result<(), DecreaseKeyError> {
        let (node, order) = self.core.set_key(handle, key)?;
        if order == Ordering::Equal || self.core.root == Some(node) {
            return Ok(());
        }
        if let Some(parent) = self.core.parent(node)
            && self.core.compare_nodes(node, parent) == Ordering::Less
        {
            self.detach_from_parent(node, parent);
            self.cascading_rank_change(parent);
            self.core.root = self.link(self.core.root, Some(node));
        }
        Ok(())
    }

    /// Removes and returns the entry associated with `handle`.
    pub fn delete(&mut self, handle: TreeHandle) -> Result<(K, V), InvalidHandle> {
        let node = self.core.validate(handle)?;
        if self.core.root == Some(node) {
            return self.pop_entry().ok_or(InvalidHandle::Stale);
        }
        let parent = self
            .core
            .parent(node)
            .expect("a non-root simple Fibonacci node has a parent");
        self.detach_from_parent(node, parent);
        self.cascading_rank_change(parent);
        for child in self.core.take_children(node) {
            self.core.root = self.link(self.core.root, Some(child));
        }
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
        self.core.clear();
    }

    #[cfg(test)]
    pub(crate) fn assert_invariants(&self) {
        self.core.assert_heap_forest(self.core.root);
        if let Some(root) = self.core.root {
            assert_eq!(self.core.node(root).parent, None);
        }
    }

    fn detach_from_parent(&mut self, node: NodeRef, parent: NodeRef) {
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
        {
            let entry = self.core.node_mut(node);
            entry.parent = None;
            entry.position = 0;
            entry.marked = false;
        }
        self.core
            .set_rank(parent, self.core.node(parent).children.len());
    }

    fn cascading_rank_change(&mut self, mut node: NodeRef) {
        while self.core.marked(node) {
            self.core.set_marked(node, false);
            self.core
                .set_rank(node, self.core.node(node).children.len());
            let Some(parent) = self.core.parent(node) else {
                return;
            };
            node = parent;
        }
        if self.core.parent(node).is_some() {
            self.core.set_marked(node, true);
        } else {
            self.core.set_marked(node, false);
        }
    }

    fn consolidate(&mut self, children: Vec<NodeRef>) -> Option<NodeRef> {
        let mut buckets = Vec::<Option<NodeRef>>::new();
        for node in children {
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
                    tree = self
                        .link(Some(other), Some(tree))
                        .expect("linked trees have a root");
                } else {
                    buckets[rank] = Some(tree);
                    break;
                }
            }
        }
        let mut root = None;
        for tree in buckets.into_iter().flatten() {
            root = self.link(root, Some(tree));
        }
        root
    }

    fn link(&mut self, first: Option<NodeRef>, second: Option<NodeRef>) -> Option<NodeRef> {
        let (first, second) = match (first, second) {
            (None, other) | (other, None) => return other,
            (Some(first), Some(second)) => (first, second),
        };
        let (parent, child) = if self.core.compare_nodes(first, second) == Ordering::Greater {
            (second, first)
        } else {
            (first, second)
        };
        self.core.push_child(parent, child);
        self.core.set_marked(child, false);
        self.core.set_rank(parent, self.core.rank(parent) + 1);
        Some(parent)
    }
}

impl<K: Ord> SimpleFibonacciHeap<K, ()> {
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

impl<K: Ord, V> SimpleFibonacciHeap<K, V> {
    /// Melds `other` into this heap, consuming the donor.
    pub fn meld(&mut self, other: Self) {
        let other_root = other.core.root;
        let other_len = other.core.len;
        self.core.take_arenas_from(other.core);
        self.core.root = self.link(self.core.root, other_root);
        self.core.len += other_len;
    }
}

impl<K: Ord, V> AddressableHeap<K, V> for SimpleFibonacciHeap<K, V> {
    type Handle = TreeHandle;

    fn insert(&mut self, key: K, value: V) -> Self::Handle {
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

impl<K: Ord, V> DecreaseKeyHeap<K, V> for SimpleFibonacciHeap<K, V> {
    fn decrease_key(&mut self, handle: Self::Handle, key: K) -> Result<(), DecreaseKeyError> {
        self.decrease_key(handle, key)
    }
}

impl<K: Ord, V> MeldableAddressableHeap<K, V> for SimpleFibonacciHeap<K, V> {
    type MeldError = Infallible;

    fn meld(&mut self, other: Self) -> Result<(), Self::MeldError> {
        self.meld(other);
        Ok(())
    }
}

crate::impl_heap_via_addressable!(SimpleFibonacciHeap);
crate::impl_meldable_heap_via_addressable!(SimpleFibonacciHeap);
