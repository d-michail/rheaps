use core::cmp::Ordering;
use core::fmt;
use std::collections::{BTreeMap, HashMap};

use crate::array::InvalidHandle;

use super::core::next_domain_id;

const TARGET_SIZE: [u64; 63] = [
    1,
    2,
    3,
    5,
    8,
    12,
    18,
    27,
    41,
    62,
    93,
    140,
    210,
    315,
    473,
    710,
    1_065,
    1_598,
    2_397,
    3_596,
    5_394,
    8_091,
    12_137,
    18_206,
    27_309,
    40_964,
    61_446,
    92_169,
    138_254,
    207_381,
    311_072,
    466_608,
    699_912,
    1_049_868,
    1_574_802,
    2_362_203,
    3_543_305,
    5_314_958,
    7_972_437,
    11_958_656,
    17_937_984,
    26_906_976,
    40_360_464,
    60_540_696,
    90_811_044,
    136_216_566,
    204_324_849,
    306_487_274,
    459_730_911,
    689_596_367,
    1_034_391_551,
    1_551_591_827,
    2_327_387_741,
    3_491_082_412,
    5_236_622_418,
    7_854_933_627,
    11_782_400_441,
    17_676_622_162,
    26_510_400_993,
    39_765_601_490,
    59_648_402_235,
    89_472_603_353,
    134_208_905_030,
];

/// An opaque handle for an entry in a binary-tree soft heap.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SoftHandle {
    pub(crate) domain: u64,
    pub(crate) slot: usize,
    pub(crate) generation: u64,
}

/// An invalid soft-heap error rate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoftHeapError {
    /// The error rate was zero or negative.
    NonPositiveErrorRate,
    /// The error rate was at least one or not a number.
    ErrorRateNotBelowOne,
}

impl fmt::Display for SoftHeapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositiveErrorRate => formatter.write_str("error rate must be positive"),
            Self::ErrorRateNotBelowOne => formatter.write_str("error rate must be less than one"),
        }
    }
}

impl std::error::Error for SoftHeapError {}

/// An error returned when binary-tree soft heaps cannot be melded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoftMeldError {
    /// The receiving heap was previously used as a meld donor.
    ReceiverConsumed,
    /// The donor heap was previously used as a meld donor.
    DonorConsumed,
    /// The heaps have incompatible error-rate rank limits.
    IncompatibleErrorRate,
}

impl fmt::Display for SoftMeldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReceiverConsumed => formatter.write_str("a meld donor cannot be reused"),
            Self::DonorConsumed => formatter.write_str("the donor heap was already consumed"),
            Self::IncompatibleErrorRate => {
                formatter.write_str("cannot meld heaps with different error rates")
            }
        }
    }
}

impl std::error::Error for SoftMeldError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SoftItemRef {
    domain: u64,
    slot: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SoftNodeRef {
    domain: u64,
    slot: usize,
}

pub(crate) struct Item<K, V> {
    key: K,
    value: V,
    next: Option<SoftItemRef>,
    prev: Option<SoftItemRef>,
    tree: Option<SoftNodeRef>,
}

impl<K, V> Item<K, V> {
    pub(crate) fn into_pair(self) -> (K, V) {
        (self.key, self.value)
    }
}

struct ItemSlot<K, V> {
    item: Option<Item<K, V>>,
    generation: u64,
}

struct ItemArena<K, V> {
    slots: Vec<ItemSlot<K, V>>,
    free_slots: Vec<usize>,
}

impl<K, V> ItemArena<K, V> {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_slots: Vec::new(),
        }
    }

    fn insert(&mut self, item: Item<K, V>) -> (usize, u64) {
        if let Some(slot) = self.free_slots.pop() {
            let generation = self.slots[slot].generation;
            self.slots[slot].item = Some(item);
            (slot, generation)
        } else {
            self.slots.push(ItemSlot {
                item: Some(item),
                generation: 0,
            });
            (self.slots.len() - 1, 0)
        }
    }

    fn remove(&mut self, slot: usize) -> Item<K, V> {
        let entry = &mut self.slots[slot];
        let item = entry.item.take().expect("soft item must be live");
        entry.generation = entry.generation.wrapping_add(1);
        self.free_slots.push(slot);
        item
    }

    fn clear(&mut self) {
        self.free_slots.clear();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.item.take().is_some() {
                slot.generation = slot.generation.wrapping_add(1);
            }
            self.free_slots.push(index);
        }
    }
}

struct Node<K> {
    rank: usize,
    parent: Option<SoftNodeRef>,
    left: Option<SoftNodeRef>,
    right: Option<SoftNodeRef>,
    c_head: Option<SoftItemRef>,
    c_tail: Option<SoftItemRef>,
    c_size: u64,
    c_key: Option<K>,
}

struct NodeArena<K> {
    nodes: Vec<Node<K>>,
}

impl<K> NodeArena<K> {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }
}

/// Shared Kaplan-Zwick binary-tree soft-heap storage.
pub(crate) struct SoftHeapCore<K, V> {
    rank_limit: usize,
    roots: Vec<SoftNodeRef>,
    item_arenas: HashMap<u64, ItemArena<K, V>>,
    node_arenas: HashMap<u64, NodeArena<K>>,
    own_domain: u64,
    len: usize,
    active: bool,
}

impl<K: Ord + Clone, V> SoftHeapCore<K, V> {
    pub(crate) fn new(error_rate: f64) -> Result<Self, SoftHeapError> {
        if error_rate <= 0.0 {
            return Err(SoftHeapError::NonPositiveErrorRate);
        }
        if !matches!(error_rate.partial_cmp(&1.0), Some(Ordering::Less)) {
            return Err(SoftHeapError::ErrorRateNotBelowOne);
        }

        let rank_limit = (-error_rate.log2()).ceil().max(0.0) as usize + 5;
        let own_domain = next_domain_id();
        let mut item_arenas = HashMap::new();
        item_arenas.insert(own_domain, ItemArena::new());
        let mut node_arenas = HashMap::new();
        node_arenas.insert(own_domain, NodeArena::new());
        Ok(Self {
            rank_limit,
            roots: Vec::new(),
            item_arenas,
            node_arenas,
            own_domain,
            len: 0,
            active: true,
        })
    }

    pub(crate) const fn rank_limit(&self) -> usize {
        self.rank_limit
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) const fn active(&self) -> bool {
        self.active
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> Result<SoftHandle, SoftMeldError> {
        if !self.active {
            return Err(SoftMeldError::ReceiverConsumed);
        }
        let item = self.insert_item(key, value);
        let node = self.insert_node(item);
        self.item_mut(item).tree = Some(node);
        self.merge_roots(vec![node]);
        self.len += 1;
        Ok(self.handle(item))
    }

    pub(crate) fn peek_item(&self) -> Option<SoftItemRef> {
        if !self.active {
            return None;
        }
        let root = self.minimum_root()?;
        self.node(root).c_head
    }

    pub(crate) fn peek_entry(&self) -> Option<(SoftHandle, &K, &V)> {
        let item = self.peek_item()?;
        let entry = self.item(item);
        Some((self.handle(item), &entry.key, &entry.value))
    }

    pub(crate) fn pop_item(&mut self) -> Option<Item<K, V>> {
        if !self.active {
            return None;
        }
        let root = self.minimum_root()?;
        let item = self.node(root).c_head.expect("root list must not be empty");
        let next = self.item(item).next;
        {
            let node = self.node_mut(root);
            node.c_head = next;
            node.c_size = node.c_size.saturating_sub(1);
            if next.is_none() {
                node.c_tail = None;
            }
        }
        if let Some(next) = next {
            let next_item = self.item_mut(next);
            next_item.prev = None;
            next_item.tree = Some(root);
        }

        if self.node(root).c_head.is_none()
            || self.node(root).c_size <= self.target_size(self.node(root).rank) / 2
        {
            if self.node(root).left.is_some() || self.node(root).right.is_some() {
                self.sift(root);
            } else if self.node(root).c_head.is_none() {
                self.detach_tree(root);
            }
        }

        self.len -= 1;
        Some(self.remove_item(item))
    }

    pub(crate) fn key(&self, handle: SoftHandle) -> Result<&K, InvalidHandle> {
        Ok(&self.item(self.validate(handle)?).key)
    }

    pub(crate) fn validate_handle(&self, handle: SoftHandle) -> Result<(), InvalidHandle> {
        self.validate(handle).map(|_| ())
    }

    pub(crate) fn value(&self, handle: SoftHandle) -> Result<&V, InvalidHandle> {
        Ok(&self.item(self.validate(handle)?).value)
    }

    pub(crate) fn set_value(&mut self, handle: SoftHandle, value: V) -> Result<(), InvalidHandle> {
        let item = self.validate(handle)?;
        self.item_mut(item).value = value;
        Ok(())
    }

    pub(crate) fn delete(&mut self, handle: SoftHandle) -> Result<Item<K, V>, InvalidHandle> {
        let item = self.validate(handle)?;
        let tree = self.owning_tree(item)?;
        if self.node(tree).c_head != Some(item) {
            let previous = self
                .item(item)
                .prev
                .expect("non-head soft item has a predecessor");
            let next = self.item(item).next;
            self.item_mut(previous).next = next;
            if let Some(next) = next {
                self.item_mut(next).prev = Some(previous);
            } else {
                self.node_mut(tree).c_tail = Some(previous);
            }
        } else {
            let next = self.item(item).next;
            self.node_mut(tree).c_head = next;
            if let Some(next) = next {
                let next_item = self.item_mut(next);
                next_item.prev = None;
                next_item.tree = Some(tree);
            } else {
                self.node_mut(tree).c_tail = None;
                self.sift(tree);
                if self.node(tree).c_head.is_none() {
                    self.detach_tree(tree);
                }
            }
        }
        self.len -= 1;
        Ok(self.remove_item(item))
    }

    pub(crate) fn clear(&mut self) {
        if !self.active {
            return;
        }
        for arena in self.item_arenas.values_mut() {
            arena.clear();
        }
        for arena in self.node_arenas.values_mut() {
            arena.nodes.clear();
        }
        self.roots.clear();
        self.len = 0;
    }

    pub(crate) fn meld_from(&mut self, other: &mut Self) {
        let roots = core::mem::take(&mut other.roots);
        self.item_arenas.extend(other.item_arenas.drain());
        self.node_arenas.extend(other.node_arenas.drain());
        self.merge_roots(roots);
        self.len += other.len;
        other.len = 0;
        other.active = false;
    }

    fn insert_item(&mut self, key: K, value: V) -> SoftItemRef {
        let arena = self
            .item_arenas
            .get_mut(&self.own_domain)
            .expect("own soft-item arena must be present");
        let (slot, _) = arena.insert(Item {
            key,
            value,
            next: None,
            prev: None,
            tree: None,
        });
        SoftItemRef {
            domain: self.own_domain,
            slot,
        }
    }

    fn insert_node(&mut self, item: SoftItemRef) -> SoftNodeRef {
        let c_key = self.item(item).key.clone();
        let arena = self
            .node_arenas
            .get_mut(&self.own_domain)
            .expect("own soft-node arena must be present");
        arena.nodes.push(Node {
            rank: 0,
            parent: None,
            left: None,
            right: None,
            c_head: Some(item),
            c_tail: Some(item),
            c_size: 1,
            c_key: Some(c_key),
        });
        SoftNodeRef {
            domain: self.own_domain,
            slot: arena.nodes.len() - 1,
        }
    }

    fn handle(&self, item: SoftItemRef) -> SoftHandle {
        let arena = self
            .item_arenas
            .get(&item.domain)
            .expect("soft item arena must be present");
        SoftHandle {
            domain: item.domain,
            slot: item.slot,
            generation: arena.slots[item.slot].generation,
        }
    }

    fn validate(&self, handle: SoftHandle) -> Result<SoftItemRef, InvalidHandle> {
        if !self.active {
            return Err(InvalidHandle::ForeignHeap);
        }
        let Some(arena) = self.item_arenas.get(&handle.domain) else {
            return Err(InvalidHandle::ForeignHeap);
        };
        let Some(slot) = arena.slots.get(handle.slot) else {
            return Err(InvalidHandle::Stale);
        };
        if slot.generation != handle.generation || slot.item.is_none() {
            return Err(InvalidHandle::Stale);
        }
        Ok(SoftItemRef {
            domain: handle.domain,
            slot: handle.slot,
        })
    }

    fn item(&self, item: SoftItemRef) -> &Item<K, V> {
        self.item_arenas
            .get(&item.domain)
            .and_then(|arena| arena.slots.get(item.slot))
            .and_then(|slot| slot.item.as_ref())
            .expect("soft item must be live")
    }

    fn item_mut(&mut self, item: SoftItemRef) -> &mut Item<K, V> {
        self.item_arenas
            .get_mut(&item.domain)
            .and_then(|arena| arena.slots.get_mut(item.slot))
            .and_then(|slot| slot.item.as_mut())
            .expect("soft item must be live")
    }

    fn remove_item(&mut self, item: SoftItemRef) -> Item<K, V> {
        self.item_arenas
            .get_mut(&item.domain)
            .expect("soft item arena must be present")
            .remove(item.slot)
    }

    fn node(&self, node: SoftNodeRef) -> &Node<K> {
        self.node_arenas
            .get(&node.domain)
            .and_then(|arena| arena.nodes.get(node.slot))
            .expect("soft node must be present")
    }

    fn node_mut(&mut self, node: SoftNodeRef) -> &mut Node<K> {
        self.node_arenas
            .get_mut(&node.domain)
            .and_then(|arena| arena.nodes.get_mut(node.slot))
            .expect("soft node must be present")
    }

    fn target_size(&self, rank: usize) -> u64 {
        if rank <= self.rank_limit {
            1
        } else {
            TARGET_SIZE
                .get(rank - self.rank_limit)
                .copied()
                .unwrap_or(u64::MAX)
        }
    }

    fn owning_tree(&self, item: SoftItemRef) -> Result<SoftNodeRef, InvalidHandle> {
        let mut head = item;
        while let Some(previous) = self.item(head).prev {
            head = previous;
        }
        self.item(head).tree.ok_or(InvalidHandle::Stale)
    }

    fn minimum_root(&self) -> Option<SoftNodeRef> {
        self.roots.iter().copied().min_by(|left, right| {
            self.node(*left)
                .c_key
                .as_ref()
                .expect("root must have a corrupted key")
                .cmp(
                    self.node(*right)
                        .c_key
                        .as_ref()
                        .expect("root must have a corrupted key"),
                )
        })
    }

    fn merge_roots(&mut self, roots: Vec<SoftNodeRef>) {
        let mut ranks = BTreeMap::<usize, Vec<SoftNodeRef>>::new();
        let existing = core::mem::take(&mut self.roots);
        for root in existing.into_iter().chain(roots) {
            self.node_mut(root).parent = None;
            ranks.entry(self.node(root).rank).or_default().push(root);
        }

        let mut result = Vec::new();
        while let Some((rank, mut trees)) = ranks.pop_first() {
            if trees.len() % 2 == 1 {
                result.push(trees.pop().expect("odd tree count has an element"));
            }
            let mut pairs = trees.into_iter();
            while let Some(left) = pairs.next() {
                let right = pairs.next().expect("trees were paired");
                let combined = self.combine(left, right);
                ranks.entry(rank + 1).or_default().push(combined);
            }
        }
        self.roots = result;
    }

    fn combine(&mut self, left: SoftNodeRef, right: SoftNodeRef) -> SoftNodeRef {
        let rank = self.node(left).rank + 1;
        debug_assert_eq!(self.node(left).rank, self.node(right).rank);
        let arena = self
            .node_arenas
            .get_mut(&self.own_domain)
            .expect("own soft-node arena must be present");
        arena.nodes.push(Node {
            rank,
            parent: None,
            left: Some(left),
            right: Some(right),
            c_head: None,
            c_tail: None,
            c_size: 0,
            c_key: None,
        });
        let node = SoftNodeRef {
            domain: self.own_domain,
            slot: arena.nodes.len() - 1,
        };
        self.node_mut(left).parent = Some(node);
        self.node_mut(right).parent = Some(node);
        self.sift(node);
        node
    }

    fn sift(&mut self, root: SoftNodeRef) {
        let mut stack = vec![root];
        while let Some(node) = stack.last().copied() {
            let (left, right, c_head, c_size, rank) = {
                let entry = self.node(node);
                (
                    entry.left,
                    entry.right,
                    entry.c_head,
                    entry.c_size,
                    entry.rank,
                )
            };
            if (left.is_none() && right.is_none())
                || (c_head.is_some() && c_size >= self.target_size(rank))
            {
                stack.pop();
                continue;
            }

            let selected_right = match (left, right) {
                (None, Some(_)) => true,
                (Some(left), Some(right)) => {
                    self.node(left)
                        .c_key
                        .as_ref()
                        .expect("non-empty child must have a corrupted key")
                        > self
                            .node(right)
                            .c_key
                            .as_ref()
                            .expect("non-empty child must have a corrupted key")
                }
                (Some(_), None) => false,
                (None, None) => unreachable!("leaf was handled above"),
            };
            if selected_right {
                let entry = self.node_mut(node);
                core::mem::swap(&mut entry.left, &mut entry.right);
            }

            let left = self.node(node).left.expect("one child must be available");
            let (child_head, child_tail, child_size, child_key) = {
                let child = self.node(left);
                (
                    child.c_head.expect("selected child must contain an item"),
                    child.c_tail.expect("selected child must contain an item"),
                    child.c_size,
                    child
                        .c_key
                        .as_ref()
                        .expect("selected child must have a corrupted key")
                        .clone(),
                )
            };
            let old_head = self.node(node).c_head;
            let old_tail = self.node(node).c_tail;
            self.item_mut(child_tail).next = old_head;
            if let Some(old_head) = old_head {
                self.item_mut(old_head).prev = Some(child_tail);
            }
            {
                let entry = self.node_mut(node);
                entry.c_head = Some(child_head);
                if old_tail.is_none() {
                    entry.c_tail = Some(child_tail);
                }
                entry.c_size = entry.c_size.saturating_add(child_size);
                entry.c_key = Some(child_key);
            }
            self.item_mut(child_head).tree = Some(node);
            {
                let child = self.node_mut(left);
                child.c_head = None;
                child.c_tail = None;
                child.c_size = 0;
                child.c_key = None;
            }

            if self.node(left).left.is_some() || self.node(left).right.is_some() {
                stack.push(left);
            } else {
                self.node_mut(node).left = None;
            }
        }
    }

    fn detach_tree(&mut self, node: SoftNodeRef) {
        if let Some(parent) = self.node(node).parent {
            let entry = self.node_mut(parent);
            if entry.left == Some(node) {
                entry.left = None;
            } else {
                debug_assert_eq!(entry.right, Some(node));
                entry.right = None;
            }
        } else if let Some(index) = self.roots.iter().position(|&root| root == node) {
            self.roots.remove(index);
        }
        self.node_mut(node).parent = None;
    }
}
