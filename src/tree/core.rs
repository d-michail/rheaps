use core::cmp::Ordering;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;

use crate::error::{DecreaseKeyError, InvalidHandle};

static NEXT_DOMAIN_ID: AtomicU64 = AtomicU64::new(1);

/// An opaque handle for an entry in an addressable tree heap.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TreeHandle {
    pub(crate) domain: u64,
    pub(crate) slot: usize,
    pub(crate) generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeldError {
    /// The receiving heap was previously used as a meld donor.
    ReceiverConsumed,
    /// The donor heap was previously used as a meld donor.
    DonorConsumed,
}

impl fmt::Display for MeldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReceiverConsumed => formatter.write_str("a meld donor cannot be reused"),
            Self::DonorConsumed => formatter.write_str("the donor heap was already consumed"),
        }
    }
}

impl std::error::Error for MeldError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct NodeRef {
    domain: u64,
    slot: usize,
}

pub(crate) struct Node<K, V> {
    pub(crate) key: K,
    pub(crate) value: V,
    pub(crate) parent: Option<NodeRef>,
    pub(crate) position: usize,
    pub(crate) children: Vec<Option<NodeRef>>,
    pub(crate) rank: usize,
    pub(crate) marked: bool,
}

struct Slot<K, V> {
    node: Option<Node<K, V>>,
    generation: u64,
}

struct Arena<K, V> {
    slots: Vec<Slot<K, V>>,
    free_slots: Vec<usize>,
}

impl<K, V> Arena<K, V> {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_slots: Vec::new(),
        }
    }

    fn insert(&mut self, node: Node<K, V>) -> (usize, u64) {
        if let Some(slot) = self.free_slots.pop() {
            let generation = self.slots[slot].generation;
            self.slots[slot].node = Some(node);
            (slot, generation)
        } else {
            self.slots.push(Slot {
                node: Some(node),
                generation: 0,
            });
            (self.slots.len() - 1, 0)
        }
    }

    fn remove(&mut self, slot: usize) -> Node<K, V> {
        let entry = &mut self.slots[slot];
        let node = entry.node.take().expect("tree reference must be live");
        entry.generation = entry.generation.wrapping_add(1);
        self.free_slots.push(slot);
        node
    }

    fn clear(&mut self) {
        self.free_slots.clear();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.node.take().is_some() {
                slot.generation = slot.generation.wrapping_add(1);
            }
            self.free_slots.push(index);
        }
    }
}

/// Shared storage and checked-handle support for addressable meldable trees.
///
/// Every heap that has contributed nodes retains a small arena in the receiver.
/// This permits a donor's handles to keep their identity after a meld without
/// moving nodes or weakening checked handle validation.
pub(crate) struct TreeCore<K, V> {
    pub(crate) root: Option<NodeRef>,
    pub(crate) len: usize,
    pub(crate) active: bool,
    own_domain: u64,
    arenas: HashMap<u64, Arena<K, V>>,
}

impl<K: Ord, V> TreeCore<K, V> {
    pub(crate) fn new() -> Self {
        let own_domain = next_domain_id();
        let mut arenas = HashMap::new();
        arenas.insert(own_domain, Arena::new());
        Self {
            root: None,
            len: 0,
            active: true,
            own_domain,
            arenas,
        }
    }

    pub(crate) fn insert_node(&mut self, key: K, value: V) -> NodeRef {
        let arena = self
            .arenas
            .get_mut(&self.own_domain)
            .expect("own tree arena must be present");
        let (slot, _) = arena.insert(Node {
            key,
            value,
            parent: None,
            position: 0,
            children: Vec::new(),
            rank: 0,
            marked: false,
        });
        NodeRef {
            domain: self.own_domain,
            slot,
        }
    }

    pub(crate) fn handle(&self, node: NodeRef) -> TreeHandle {
        let arena = self
            .arenas
            .get(&node.domain)
            .expect("tree reference must have an arena");
        TreeHandle {
            domain: node.domain,
            slot: node.slot,
            generation: arena.slots[node.slot].generation,
        }
    }

    pub(crate) fn validate(&self, handle: TreeHandle) -> Result<NodeRef, InvalidHandle> {
        if !self.active {
            return Err(InvalidHandle::ForeignHeap);
        }
        let Some(arena) = self.arenas.get(&handle.domain) else {
            return Err(InvalidHandle::ForeignHeap);
        };
        let Some(slot) = arena.slots.get(handle.slot) else {
            return Err(InvalidHandle::Stale);
        };
        if slot.generation != handle.generation || slot.node.is_none() {
            return Err(InvalidHandle::Stale);
        }
        Ok(NodeRef {
            domain: handle.domain,
            slot: handle.slot,
        })
    }

    pub(crate) fn key(&self, handle: TreeHandle) -> Result<&K, InvalidHandle> {
        Ok(&self.node(self.validate(handle)?).key)
    }

    pub(crate) fn value(&self, handle: TreeHandle) -> Result<&V, InvalidHandle> {
        Ok(&self.node(self.validate(handle)?).value)
    }

    pub(crate) fn value_mut(&mut self, handle: TreeHandle) -> Result<&mut V, InvalidHandle> {
        let node = self.validate(handle)?;
        Ok(&mut self.node_mut(node).value)
    }

    pub(crate) fn compare_nodes(&self, left: NodeRef, right: NodeRef) -> Ordering {
        self.node(left).key.cmp(&self.node(right).key)
    }

    pub(crate) fn compare_key(&self, key: &K, node: NodeRef) -> Ordering {
        key.cmp(&self.node(node).key)
    }

    pub(crate) fn set_key(
        &mut self,
        handle: TreeHandle,
        key: K,
    ) -> Result<(NodeRef, Ordering), DecreaseKeyError> {
        let node = self
            .validate(handle)
            .map_err(DecreaseKeyError::InvalidHandle)?;
        let order = self.compare_key(&key, node);
        if order == Ordering::Greater {
            return Err(DecreaseKeyError::NotDecreased);
        }
        self.node_mut(node).key = key;
        Ok((node, order))
    }

    pub(crate) fn node(&self, node: NodeRef) -> &Node<K, V> {
        self.arenas
            .get(&node.domain)
            .and_then(|arena| arena.slots.get(node.slot))
            .and_then(|slot| slot.node.as_ref())
            .expect("tree reference must be live")
    }

    pub(crate) fn node_mut(&mut self, node: NodeRef) -> &mut Node<K, V> {
        self.arenas
            .get_mut(&node.domain)
            .and_then(|arena| arena.slots.get_mut(node.slot))
            .and_then(|slot| slot.node.as_mut())
            .expect("tree reference must be live")
    }

    pub(crate) fn remove_node(&mut self, node: NodeRef) -> Node<K, V> {
        self.arenas
            .get_mut(&node.domain)
            .expect("tree reference must have an arena")
            .remove(node.slot)
    }

    pub(crate) fn child(&self, node: NodeRef, position: usize) -> Option<NodeRef> {
        self.node(node).children.get(position).copied().flatten()
    }

    pub(crate) fn take_child(&mut self, node: NodeRef, position: usize) -> Option<NodeRef> {
        let child = self.node_mut(node).children.get_mut(position)?.take();
        if let Some(child) = child {
            self.node_mut(child).parent = None;
        }
        child
    }

    pub(crate) fn set_child(&mut self, parent: NodeRef, position: usize, child: Option<NodeRef>) {
        {
            let node = self.node_mut(parent);
            if node.children.len() <= position {
                node.children.resize(position + 1, None);
            }
            node.children[position] = child;
        }
        if let Some(child) = child {
            let child_node = self.node_mut(child);
            child_node.parent = Some(parent);
            child_node.position = position;
        }
    }

    pub(crate) fn push_child(&mut self, parent: NodeRef, child: NodeRef) {
        let position = self.node(parent).children.len();
        self.set_child(parent, position, Some(child));
    }

    pub(crate) fn take_children(&mut self, node: NodeRef) -> Vec<NodeRef> {
        let children = core::mem::take(&mut self.node_mut(node).children);
        children
            .into_iter()
            .flatten()
            .inspect(|child| self.node_mut(*child).parent = None)
            .collect()
    }

    pub(crate) fn parent(&self, node: NodeRef) -> Option<NodeRef> {
        self.node(node).parent
    }

    pub(crate) fn position(&self, node: NodeRef) -> usize {
        self.node(node).position
    }

    pub(crate) fn set_rank(&mut self, node: NodeRef, rank: usize) {
        self.node_mut(node).rank = rank;
    }

    pub(crate) fn rank(&self, node: NodeRef) -> usize {
        self.node(node).rank
    }

    pub(crate) fn set_marked(&mut self, node: NodeRef, marked: bool) {
        self.node_mut(node).marked = marked;
    }

    pub(crate) fn marked(&self, node: NodeRef) -> bool {
        self.node(node).marked
    }

    pub(crate) fn clear(&mut self) {
        for arena in self.arenas.values_mut() {
            arena.clear();
        }
        self.root = None;
        self.len = 0;
    }

    pub(crate) fn take_arenas_from(&mut self, other: &mut Self) {
        self.arenas.extend(other.arenas.drain());
    }

    #[cfg(test)]
    pub(crate) fn assert_heap_forest(&self, roots: impl IntoIterator<Item = NodeRef>) {
        if !self.active {
            return;
        }

        let roots = roots.into_iter().collect::<Vec<_>>();
        let mut visited = HashSet::new();
        let mut stack = roots
            .iter()
            .copied()
            .map(|root| (root, None))
            .collect::<Vec<_>>();
        while let Some((node, parent)) = stack.pop() {
            assert!(visited.insert(node), "a tree node appears more than once");
            let entry = self.node(node);
            assert_eq!(entry.parent, parent, "parent link is inconsistent");
            for (position, child) in entry.children.iter().enumerate() {
                if let Some(child) = child {
                    let child_entry = self.node(*child);
                    assert_eq!(
                        child_entry.position, position,
                        "child position is inconsistent"
                    );
                    assert!(entry.key <= child_entry.key, "heap order is violated");
                    stack.push((*child, Some(node)));
                }
            }
        }
        assert_eq!(visited.len(), self.len, "a live node is unreachable");
    }
}

pub(crate) fn next_domain_id() -> u64 {
    let id = NEXT_DOMAIN_ID.fetch_add(1, AtomicOrdering::Relaxed);
    if id == 0 {
        NEXT_DOMAIN_ID.fetch_add(1, AtomicOrdering::Relaxed)
    } else {
        id
    }
}
