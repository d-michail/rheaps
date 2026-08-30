use core::cmp::Ordering;
use core::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::collections::HashMap;

use crate::error::{DecreaseKeyError, InvalidHandle};
use crate::tree::MeldError;
use crate::{AddressableHeap, DecreaseKeyHeap, MeldableAddressableHeap};

static NEXT_DOMAIN_ID: AtomicU64 = AtomicU64::new(1);

/// An opaque capability for a live entry in a [`HollowHeap`].
///
/// A handle remains usable after its creating heap is melded into another
/// hollow heap. It becomes stale after its entry is removed or the receiving
/// heap is cleared.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HollowHandle {
    domain: u64,
    slot: usize,
    generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct NodeRef {
    domain: u64,
    slot: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ItemRef {
    domain: u64,
    slot: usize,
}

struct Item<K, V> {
    key: K,
    value: V,
    node: NodeRef,
}

/// A hollow-heap DAG node.
///
/// A node holds an item only while it is full. `child`, `next`, and
/// `second_parent` are indices into stable slot arenas, avoiding self
/// references and unsafe pointers.
struct Node {
    item: Option<ItemRef>,
    child: Option<NodeRef>,
    next: Option<NodeRef>,
    second_parent: Option<NodeRef>,
    rank: usize,
}

struct Slot<T> {
    value: Option<T>,
    generation: u64,
}

struct Arena<T> {
    slots: Vec<Slot<T>>,
    free_slots: Vec<usize>,
}

impl<T> Arena<T> {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_slots: Vec::new(),
        }
    }

    fn insert(&mut self, value: T) -> (usize, u64) {
        if let Some(slot) = self.free_slots.pop() {
            let entry = &mut self.slots[slot];
            debug_assert!(entry.value.is_none());
            entry.value = Some(value);
            (slot, entry.generation)
        } else {
            self.slots.push(Slot {
                value: Some(value),
                generation: 0,
            });
            (self.slots.len() - 1, 0)
        }
    }

    fn get(&self, slot: usize) -> Option<&T> {
        self.slots.get(slot)?.value.as_ref()
    }

    fn get_mut(&mut self, slot: usize) -> Option<&mut T> {
        self.slots.get_mut(slot)?.value.as_mut()
    }

    fn generation(&self, slot: usize) -> Option<u64> {
        self.slots.get(slot).map(|entry| entry.generation)
    }

    fn remove(&mut self, slot: usize) -> T {
        let entry = self
            .slots
            .get_mut(slot)
            .expect("slot reference must belong to its arena");
        let value = entry.value.take().expect("slot reference must be live");
        entry.generation = entry.generation.wrapping_add(1);
        self.free_slots.push(slot);
        value
    }

    fn clear(&mut self) {
        self.free_slots.clear();
        for (slot, entry) in self.slots.iter_mut().enumerate() {
            if entry.value.take().is_some() {
                entry.generation = entry.generation.wrapping_add(1);
            }
            self.free_slots.push(slot);
        }
    }

    #[cfg(test)]
    fn live_len(&self) -> usize {
        self.slots.len().saturating_sub(self.free_slots.len())
    }
}

struct DomainArena<K, V> {
    nodes: Arena<Node>,
    items: Arena<Item<K, V>>,
}

impl<K, V> DomainArena<K, V> {
    fn new() -> Self {
        Self {
            nodes: Arena::new(),
            items: Arena::new(),
        }
    }

    fn clear(&mut self) {
        self.nodes.clear();
        self.items.clear();
    }
}

/// An addressable, meldable hollow heap.
///
/// Insertions, decreases, and melds are amortized `O(1)`; deleting the
/// minimum and deleting an entry are amortized `O(log n)`. A key decrease
/// creates a new full node for the entry while retaining the old node as a
/// hollow node. Hollow nodes are reclaimed lazily when a minimum is deleted.
///
/// The implementation uses slot arenas rather than pointers. Entries and
/// nodes have independent stable storage, so opaque handles survive moves and
/// successful melds without unsafe code or a binary-heap substitute.
pub struct HollowHeap<K, V = ()> {
    root: Option<NodeRef>,
    len: usize,
    nodes: usize,
    active: bool,
    own_domain: u64,
    arenas: HashMap<u64, DomainArena<K, V>>,
}

impl<K: Ord, V> HollowHeap<K, V> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        let own_domain = next_domain_id();
        let mut arenas = HashMap::new();
        arenas.insert(own_domain, DomainArena::new());
        Self {
            root: None,
            len: 0,
            nodes: 0,
            active: true,
            own_domain,
            arenas,
        }
    }
}

impl<K: Ord, V> Default for HollowHeap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> HollowHeap<K, V> {
    /// Inserts an entry and returns its checked handle.
    ///
    /// # Panics
    ///
    /// Panics when this heap was consumed as a meld donor. Use
    /// [`Self::try_insert`] to handle that condition explicitly.
    pub fn insert(&mut self, key: K, value: V) -> HollowHandle {
        self.try_insert(key, value)
            .expect("a meld donor cannot accept new entries")
    }

    /// Inserts an entry unless this heap was consumed as a meld donor.
    pub fn try_insert(&mut self, key: K, value: V) -> Result<HollowHandle, MeldError> {
        if !self.active {
            return Err(MeldError::ReceiverConsumed);
        }

        let item = self.insert_item(key, value);
        let node = self.insert_node(Node {
            item: Some(item),
            child: None,
            next: None,
            second_parent: None,
            rank: 0,
        });
        self.item_mut(item).node = node;
        self.root = self.link_options(self.root, Some(node));
        self.len += 1;
        Ok(self.handle(item))
    }

    /// Returns the handle, key, and value of a minimum entry.
    #[must_use]
    pub fn peek_entry(&self) -> Option<(HollowHandle, &K, &V)> {
        if !self.active {
            return None;
        }
        let item = self.root.and_then(|root| self.node(root).item)?;
        let item_entry = self.item(item);
        Some((self.handle(item), &item_entry.key, &item_entry.value))
    }

    /// Removes and returns a minimum key-value pair.
    pub fn pop_entry(&mut self) -> Option<(K, V)> {
        if !self.active {
            return None;
        }
        let root = self.root?;
        let item = self
            .node(root)
            .item
            .expect("a nonempty hollow heap must have a full root");
        Some(self.remove_item(item))
    }

    /// Returns the key associated with `handle`.
    pub fn key(&self, handle: HollowHandle) -> Result<&K, InvalidHandle> {
        let item = self.validate(handle)?;
        Ok(&self.item(item).key)
    }

    /// Returns the value associated with `handle`.
    pub fn value(&self, handle: HollowHandle) -> Result<&V, InvalidHandle> {
        let item = self.validate(handle)?;
        Ok(&self.item(item).value)
    }

    /// Returns mutable access to the value associated with `handle`.
    pub fn value_mut(&mut self, handle: HollowHandle) -> Result<&mut V, InvalidHandle> {
        let item = self.validate(handle)?;
        Ok(&mut self.item_mut(item).value)
    }

    /// Decreases an entry's key.
    ///
    /// A non-root decrease moves the entry to a new node and leaves its
    /// previous node hollow for lazy reclamation.
    pub fn decrease_key(&mut self, handle: HollowHandle, key: K) -> Result<(), DecreaseKeyError> {
        let item = self
            .validate(handle)
            .map_err(DecreaseKeyError::InvalidHandle)?;
        let order = key.cmp(&self.item(item).key);
        if order == Ordering::Greater {
            return Err(DecreaseKeyError::NotDecreased);
        }

        let old_node = self.item(item).node;
        if order == Ordering::Equal || self.root == Some(old_node) {
            self.item_mut(item).key = key;
            return Ok(());
        }

        let rank = self.node(old_node).rank;
        let new_node = self.insert_node(Node {
            item: Some(item),
            child: Some(old_node),
            next: None,
            second_parent: None,
            rank: rank.saturating_sub(2),
        });
        self.node_mut(old_node).item = None;
        self.node_mut(old_node).second_parent = Some(new_node);
        {
            let item_entry = self.item_mut(item);
            item_entry.key = key;
            item_entry.node = new_node;
        }
        self.root = self.link_options(self.root, Some(new_node));
        Ok(())
    }

    /// Removes and returns the entry associated with `handle`.
    ///
    /// Non-minimum deletion only makes the entry's node hollow. It is
    /// reclaimed later by a minimum deletion.
    pub fn delete(&mut self, handle: HollowHandle) -> Result<(K, V), InvalidHandle> {
        let item = self.validate(handle)?;
        Ok(self.remove_item(item))
    }

    /// Returns the number of live entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the heap contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Removes every entry, invalidates every handle, and reclaims all nodes.
    pub fn clear(&mut self) {
        if !self.active {
            return;
        }
        for arena in self.arenas.values_mut() {
            arena.clear();
        }
        self.root = None;
        self.len = 0;
        self.nodes = 0;
    }

    fn insert_item(&mut self, key: K, value: V) -> ItemRef {
        let arena = self
            .arenas
            .get_mut(&self.own_domain)
            .expect("own hollow-heap arena must be present");
        let (slot, _) = arena.items.insert(Item {
            key,
            value,
            node: NodeRef {
                domain: self.own_domain,
                slot: 0,
            },
        });
        ItemRef {
            domain: self.own_domain,
            slot,
        }
    }

    fn insert_node(&mut self, node: Node) -> NodeRef {
        let arena = self
            .arenas
            .get_mut(&self.own_domain)
            .expect("own hollow-heap arena must be present");
        let (slot, _) = arena.nodes.insert(node);
        self.nodes += 1;
        NodeRef {
            domain: self.own_domain,
            slot,
        }
    }

    fn handle(&self, item: ItemRef) -> HollowHandle {
        let generation = self
            .arenas
            .get(&item.domain)
            .and_then(|arena| arena.items.generation(item.slot))
            .expect("live item must have a slot");
        HollowHandle {
            domain: item.domain,
            slot: item.slot,
            generation,
        }
    }

    fn validate(&self, handle: HollowHandle) -> Result<ItemRef, InvalidHandle> {
        if !self.active {
            return Err(InvalidHandle::ForeignHeap);
        }
        let Some(arena) = self.arenas.get(&handle.domain) else {
            return Err(InvalidHandle::ForeignHeap);
        };
        let Some(generation) = arena.items.generation(handle.slot) else {
            return Err(InvalidHandle::Stale);
        };
        if generation != handle.generation || arena.items.get(handle.slot).is_none() {
            return Err(InvalidHandle::Stale);
        }
        Ok(ItemRef {
            domain: handle.domain,
            slot: handle.slot,
        })
    }

    fn item(&self, item: ItemRef) -> &Item<K, V> {
        self.arenas
            .get(&item.domain)
            .and_then(|arena| arena.items.get(item.slot))
            .expect("item reference must be live")
    }

    fn item_mut(&mut self, item: ItemRef) -> &mut Item<K, V> {
        self.arenas
            .get_mut(&item.domain)
            .and_then(|arena| arena.items.get_mut(item.slot))
            .expect("item reference must be live")
    }

    fn node(&self, node: NodeRef) -> &Node {
        self.arenas
            .get(&node.domain)
            .and_then(|arena| arena.nodes.get(node.slot))
            .expect("node reference must be live")
    }

    fn node_mut(&mut self, node: NodeRef) -> &mut Node {
        self.arenas
            .get_mut(&node.domain)
            .and_then(|arena| arena.nodes.get_mut(node.slot))
            .expect("node reference must be live")
    }

    fn take_item(&mut self, item: ItemRef) -> Item<K, V> {
        self.arenas
            .get_mut(&item.domain)
            .expect("item reference must have an arena")
            .items
            .remove(item.slot)
    }

    fn remove_node(&mut self, node: NodeRef) {
        self.arenas
            .get_mut(&node.domain)
            .expect("node reference must have an arena")
            .nodes
            .remove(node.slot);
        self.nodes -= 1;
    }

    fn remove_item(&mut self, item: ItemRef) -> (K, V) {
        let node = self.item(item).node;
        debug_assert_eq!(self.node(node).item, Some(item));
        self.node_mut(node).item = None;
        self.len -= 1;
        if self.root == Some(node) {
            self.delete_minimum();
        }
        let item = self.take_item(item);
        (item.key, item.value)
    }

    fn delete_minimum(&mut self) {
        debug_assert!(self.root.is_some());
        debug_assert!(self.root.is_some_and(|root| self.node(root).item.is_none()));

        let mut roots = self.root.take();
        let mut buckets = Vec::<Option<NodeRef>>::new();
        let mut max_rank = None;

        while let Some(root) = roots {
            roots = self.node(root).next;
            self.node_mut(root).next = None;

            let mut child = self.node_mut(root).child.take();
            while let Some(node) = child {
                child = self.node(node).next;
                self.node_mut(node).next = None;

                if self.node(node).item.is_none() {
                    match self.node(node).second_parent {
                        None => {
                            self.node_mut(node).next = roots;
                            roots = Some(node);
                        }
                        Some(second_parent) if second_parent == root => {
                            self.node_mut(node).second_parent = None;
                        }
                        Some(_) => {
                            self.node_mut(node).second_parent = None;
                        }
                    }
                } else {
                    let rank = self.do_ranked_links(node, &mut buckets);
                    max_rank = Some(max_rank.map_or(rank, |maximum: usize| maximum.max(rank)));
                }
            }

            debug_assert!(self.node(root).item.is_none());
            self.remove_node(root);
        }

        if let Some(max_rank) = max_rank {
            for bucket in buckets.iter_mut().take(max_rank + 1) {
                if let Some(node) = bucket.take() {
                    self.root = self.link_options(self.root, Some(node));
                }
            }
        }
    }

    fn do_ranked_links(&mut self, mut node: NodeRef, buckets: &mut Vec<Option<NodeRef>>) -> usize {
        loop {
            let rank = self.node(node).rank;
            if buckets.len() <= rank {
                buckets.resize(rank + 1, None);
            }
            let Some(other) = buckets[rank].take() else {
                buckets[rank] = Some(node);
                return rank;
            };
            node = self.link(node, other);
            self.node_mut(node).rank += 1;
        }
    }

    fn link_options(&mut self, first: Option<NodeRef>, second: Option<NodeRef>) -> Option<NodeRef> {
        match (first, second) {
            (None, other) | (other, None) => other,
            (Some(first), Some(second)) => Some(self.link(first, second)),
        }
    }

    fn link(&mut self, first: NodeRef, second: NodeRef) -> NodeRef {
        let (parent, child) = if self.compare_nodes(first, second) == Ordering::Greater {
            (second, first)
        } else {
            (first, second)
        };
        let first_child = self.node(parent).child;
        {
            let child_node = self.node_mut(child);
            child_node.next = first_child;
        }
        self.node_mut(parent).child = Some(child);
        parent
    }

    fn compare_nodes(&self, first: NodeRef, second: NodeRef) -> Ordering {
        let first_item = self
            .node(first)
            .item
            .expect("only full hollow-heap nodes can be linked");
        let second_item = self
            .node(second)
            .item
            .expect("only full hollow-heap nodes can be linked");
        self.item(first_item).key.cmp(&self.item(second_item).key)
    }

    #[cfg(test)]
    pub(crate) fn assert_invariants(&self) {
        if !self.active {
            return;
        }
        assert_eq!(
            self.nodes,
            self.arenas
                .values()
                .map(|arena| arena.nodes.live_len())
                .sum()
        );
        assert_eq!(
            self.len,
            self.arenas
                .values()
                .map(|arena| arena.items.live_len())
                .sum()
        );
        assert_eq!(self.root.is_none(), self.len == 0);

        if let Some(root) = self.root {
            assert!(
                self.node(root).item.is_some(),
                "a nonempty heap must have a full minimum root"
            );
        }

        for arena in self.arenas.values() {
            for node in arena
                .nodes
                .slots
                .iter()
                .filter_map(|slot| slot.value.as_ref())
            {
                if let Some(item) = node.item {
                    assert_eq!(self.item(item).node, self.find_node_for_item(item));
                }
            }
        }
    }

    #[cfg(test)]
    fn find_node_for_item(&self, item: ItemRef) -> NodeRef {
        for (&domain, arena) in &self.arenas {
            for (slot, entry) in arena.nodes.slots.iter().enumerate() {
                if entry.value.as_ref().and_then(|node| node.item) == Some(item) {
                    return NodeRef { domain, slot };
                }
            }
        }
        panic!("every live item must be held by a full node");
    }
}

impl<K: Ord> HollowHeap<K, ()> {
    /// Inserts a key into this value-less heap.
    ///
    /// # Panics
    ///
    /// Panics when this heap was consumed as a meld donor.
    pub fn push(&mut self, key: K) {
        self.insert(key, ());
    }

    /// Returns a reference to a minimum key.
    #[must_use]
    pub fn peek(&self) -> Option<&K> {
        self.peek_entry().map(|(_, key, _)| key)
    }

    /// Removes and returns a minimum key.
    pub fn pop(&mut self) -> Option<K> {
        self.pop_entry().map(|(key, ())| key)
    }
}

impl<K: Ord, V> HollowHeap<K, V> {
    /// Melds `other` into this heap, consuming the donor on success.
    ///
    /// All donor handles become valid handles of this heap without moving
    /// their entries. The donor can no longer be queried or mutated.
    pub fn meld(&mut self, other: &mut Self) -> Result<(), MeldError> {
        if !self.active {
            return Err(MeldError::ReceiverConsumed);
        }
        if !other.active {
            return Err(MeldError::DonorConsumed);
        }
        let other_root = other.root;
        self.arenas.extend(other.arenas.drain());
        self.root = self.link_options(self.root, other_root);
        self.len += other.len;
        self.nodes += other.nodes;

        other.root = None;
        other.len = 0;
        other.nodes = 0;
        other.active = false;
        Ok(())
    }
}

impl<K: Ord, V> AddressableHeap<K, V> for HollowHeap<K, V> {
    type Handle = HollowHandle;

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

impl<K: Ord, V> DecreaseKeyHeap<K, V> for HollowHeap<K, V> {
    fn decrease_key(&mut self, handle: Self::Handle, key: K) -> Result<(), DecreaseKeyError> {
        Self::decrease_key(self, handle, key)
    }
}

impl<K: Ord, V> MeldableAddressableHeap<K, V> for HollowHeap<K, V> {
    type MeldError = MeldError;

    fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
        Self::meld(self, other)
    }
}

fn next_domain_id() -> u64 {
    let id = NEXT_DOMAIN_ID.fetch_add(1, AtomicOrdering::Relaxed);
    if id == 0 {
        NEXT_DOMAIN_ID.fetch_add(1, AtomicOrdering::Relaxed)
    } else {
        id
    }
}

crate::impl_heap_via_addressable!(HollowHeap);
crate::impl_meldable_heap_via_addressable!(HollowHeap);
