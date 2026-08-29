use core::cmp::Reverse;
use core::marker::PhantomData;
use std::collections::HashMap;

use crate::array::{DecreaseKeyError, IncreaseKeyError, InvalidHandle};
use crate::{
    AddressableHeap, DoubleEndedAddressableHeap, MeldableAddressableHeap,
    MeldableDoubleEndedAddressableHeap,
};

use super::core::{MeldError, TreeHandle, next_domain_id};
use super::{FibonacciHeap, PairingHeap};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct InnerRecord {
    outer: ReflectedHandle,
    other: Option<TreeHandle>,
}

/// Selects the meldable tree heaps backing a reflected heap.
///
/// Not public: [`ReflectedFibonacciHeap`] and [`ReflectedPairingHeap`] are
/// the only two instantiations, and they wrap [`ReflectedHeap`] behind a
/// concrete, non-generic public type instead of exposing this backend
/// parameter.
trait ReflectedHeapBackend<K: Ord> {
    /// The min-oriented inner heap.
    type Min: AddressableHeap<K, InnerRecord, Handle = TreeHandle>;
    /// The max-oriented inner heap.
    type Max: AddressableHeap<Reverse<K>, InnerRecord, Handle = TreeHandle>;

    /// Constructs the paired inner heaps.
    fn new() -> (Self::Min, Self::Max);
}

/// Uses Fibonacci heaps in a reflected heap.
#[derive(Clone, Copy, Debug, Default)]
struct FibonacciReflectedBackend;

impl<K: Ord> ReflectedHeapBackend<K> for FibonacciReflectedBackend {
    type Min = FibonacciHeap<K, InnerRecord>;
    type Max = FibonacciHeap<Reverse<K>, InnerRecord>;

    fn new() -> (Self::Min, Self::Max) {
        (FibonacciHeap::new(), FibonacciHeap::new())
    }
}

/// Uses pairing heaps in a reflected heap.
#[derive(Clone, Copy, Debug, Default)]
struct PairingReflectedBackend;

impl<K: Ord> ReflectedHeapBackend<K> for PairingReflectedBackend {
    type Min = PairingHeap<K, InnerRecord>;
    type Max = PairingHeap<Reverse<K>, InnerRecord>;

    fn new() -> (Self::Min, Self::Max) {
        (PairingHeap::new(), PairingHeap::new())
    }
}

/// An opaque handle for an entry in a reflected double-ended heap.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReflectedHandle {
    domain: u64,
    slot: usize,
    generation: u64,
}

struct OuterEntry<K, V> {
    key: Option<K>,
    value: V,
    location: Option<Location>,
}

#[derive(Clone, Copy)]
enum Location {
    Min(TreeHandle),
    Max(TreeHandle),
}

struct OuterSlot<K, V> {
    entry: Option<OuterEntry<K, V>>,
    generation: u64,
}

struct OuterArena<K, V> {
    slots: Vec<OuterSlot<K, V>>,
    free_slots: Vec<usize>,
}

impl<K, V> OuterArena<K, V> {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_slots: Vec::new(),
        }
    }

    fn insert(&mut self, entry: OuterEntry<K, V>) -> (usize, u64) {
        if let Some(slot) = self.free_slots.pop() {
            let generation = self.slots[slot].generation;
            self.slots[slot].entry = Some(entry);
            (slot, generation)
        } else {
            self.slots.push(OuterSlot {
                entry: Some(entry),
                generation: 0,
            });
            (self.slots.len() - 1, 0)
        }
    }

    fn remove(&mut self, slot: usize) -> OuterEntry<K, V> {
        let entry = &mut self.slots[slot];
        let value = entry.entry.take().expect("reflected entry must be live");
        entry.generation = entry.generation.wrapping_add(1);
        self.free_slots.push(slot);
        value
    }

    fn clear(&mut self) {
        self.free_slots.clear();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.entry.take().is_some() {
                slot.generation = slot.generation.wrapping_add(1);
            }
            self.free_slots.push(index);
        }
    }
}

/// A reflected, double-ended addressable heap, generic over which meldable
/// tree heap backs it.
///
/// The implementation keeps each paired entry in either a min-oriented or a
/// max-oriented meldable heap. One unpaired entry is retained when the number
/// of elements is odd. Not public: [`ReflectedFibonacciHeap`] and
/// [`ReflectedPairingHeap`] each wrap this type with a fixed backend instead
/// of exposing the backend parameter.
struct ReflectedHeap<K, V, B>
where
    K: Ord,
    B: ReflectedHeapBackend<K>,
{
    min_heap: B::Min,
    max_heap: B::Max,
    free: Option<ReflectedHandle>,
    len: usize,
    active: bool,
    own_domain: u64,
    arenas: HashMap<u64, OuterArena<K, V>>,
    backend: PhantomData<B>,
}

impl<K: Ord, V, B> ReflectedHeap<K, V, B>
where
    B: ReflectedHeapBackend<K>,
{
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        let (min_heap, max_heap) = B::new();
        let own_domain = next_domain_id();
        let mut arenas = HashMap::new();
        arenas.insert(own_domain, OuterArena::new());
        Self {
            min_heap,
            max_heap,
            free: None,
            len: 0,
            active: true,
            own_domain,
            arenas,
            backend: PhantomData,
        }
    }

    /// Inserts an entry unless this heap was consumed as a meld donor.
    pub fn try_insert(&mut self, key: K, value: V) -> Result<ReflectedHandle, MeldError> {
        if !self.active {
            return Err(MeldError::ReceiverConsumed);
        }
        let handle = self.insert_outer(key, value);
        if let Some(free) = self.free.take() {
            self.insert_pair(handle, free);
        } else {
            self.free = Some(handle);
        }
        self.len += 1;
        Ok(handle)
    }

    /// Inserts an entry and returns a checked handle.
    pub fn insert(&mut self, key: K, value: V) -> ReflectedHandle {
        self.try_insert(key, value)
            .expect("a meld donor cannot accept new entries")
    }

    /// Returns the handle, key, and value of a minimum entry.
    #[must_use]
    pub fn peek_entry(&self) -> Option<(ReflectedHandle, &K, &V)> {
        let handle = self.minimum_handle()?;
        let key = self.key(handle).expect("minimum handle must be live");
        let value = self.value(handle).expect("minimum handle must be live");
        Some((handle, key, value))
    }

    /// Returns the handle, key, and value of a maximum entry.
    #[must_use]
    pub fn peek_max_entry(&self) -> Option<(ReflectedHandle, &K, &V)> {
        let handle = self.maximum_handle()?;
        let key = self.key(handle).expect("maximum handle must be live");
        let value = self.value(handle).expect("maximum handle must be live");
        Some((handle, key, value))
    }

    /// Removes and returns a minimum entry.
    pub fn pop_entry(&mut self) -> Option<(K, V)> {
        let handle = self.minimum_handle()?;
        Some(self.delete(handle).expect("minimum handle must be live"))
    }

    /// Removes and returns a maximum entry.
    pub fn pop_max_entry(&mut self) -> Option<(K, V)> {
        let handle = self.maximum_handle()?;
        Some(self.delete(handle).expect("maximum handle must be live"))
    }

    /// Returns the key identified by `handle`.
    pub fn key(&self, handle: ReflectedHandle) -> Result<&K, InvalidHandle> {
        let entry = self.outer(self.validate(handle)?);
        match entry.location {
            None => Ok(entry.key.as_ref().expect("free entry must own its key")),
            Some(Location::Min(inner)) => {
                self.min_heap.key(inner).map_err(|_| InvalidHandle::Stale)
            }
            Some(Location::Max(inner)) => self
                .max_heap
                .key(inner)
                .map(|key| &key.0)
                .map_err(|_| InvalidHandle::Stale),
        }
    }

    /// Returns the value identified by `handle`.
    pub fn value(&self, handle: ReflectedHandle) -> Result<&V, InvalidHandle> {
        Ok(&self.outer(self.validate(handle)?).value)
    }

    /// Returns mutable access to the value identified by `handle`.
    pub fn value_mut(&mut self, handle: ReflectedHandle) -> Result<&mut V, InvalidHandle> {
        let outer = self.validate(handle)?;
        Ok(&mut self.outer_mut(outer).value)
    }

    /// Decreases the key identified by `handle`.
    pub fn decrease_key(
        &mut self,
        handle: ReflectedHandle,
        key: K,
    ) -> Result<(), DecreaseKeyError> {
        self.validate(handle)
            .map_err(DecreaseKeyError::InvalidHandle)?;
        let order = key.cmp(self.key(handle).expect("handle was validated"));
        if order.is_gt() {
            return Err(DecreaseKeyError::NotDecreased);
        }
        match self.outer(handle).location {
            None => self.outer_mut(handle).key = Some(key),
            Some(Location::Min(inner)) => self
                .min_heap
                .decrease_key(inner, key)
                .expect("minimum inner handle must be live"),
            Some(Location::Max(inner)) if order.is_eq() => self
                .max_heap
                .decrease_key(inner, Reverse(key))
                .expect("maximum inner handle must be live"),
            Some(Location::Max(inner)) => self.repair_changed_maximum(handle, inner, key),
        }
        Ok(())
    }

    /// Increases the key identified by `handle`.
    pub fn increase_key(
        &mut self,
        handle: ReflectedHandle,
        key: K,
    ) -> Result<(), IncreaseKeyError> {
        self.validate(handle)
            .map_err(IncreaseKeyError::InvalidHandle)?;
        let order = key.cmp(self.key(handle).expect("handle was validated"));
        if order.is_lt() {
            return Err(IncreaseKeyError::NotIncreased);
        }
        match self.outer(handle).location {
            None => self.outer_mut(handle).key = Some(key),
            Some(Location::Max(inner)) => self
                .max_heap
                .decrease_key(inner, Reverse(key))
                .expect("maximum inner handle must be live"),
            Some(Location::Min(inner)) if order.is_eq() => self
                .min_heap
                .decrease_key(inner, key)
                .expect("minimum inner handle must be live"),
            Some(Location::Min(inner)) => self.repair_changed_minimum(handle, inner, key),
        }
        Ok(())
    }

    /// Removes and returns the entry identified by `handle`.
    pub fn delete(&mut self, handle: ReflectedHandle) -> Result<(K, V), InvalidHandle> {
        self.validate(handle)?;
        let location = self.outer(handle).location;
        let (key, partner) = match location {
            None => {
                debug_assert_eq!(self.free, Some(handle));
                self.free = None;
                let entry = self.remove_outer(handle);
                self.len -= 1;
                return Ok((entry.key.expect("free entry must own its key"), entry.value));
            }
            Some(Location::Min(inner)) => {
                let (key, record) = self
                    .min_heap
                    .delete(inner)
                    .expect("minimum inner handle must be live");
                let partner = record.other.expect("paired inner handle must be set");
                let (partner_key, partner_record) = self
                    .max_heap
                    .delete(partner)
                    .expect("maximum inner handle must be live");
                debug_assert_eq!(record.outer, handle);
                (key, (partner_record.outer, partner_key.0))
            }
            Some(Location::Max(inner)) => {
                let (key, record) = self
                    .max_heap
                    .delete(inner)
                    .expect("maximum inner handle must be live");
                let partner = record.other.expect("paired inner handle must be set");
                let (partner_key, partner_record) = self
                    .min_heap
                    .delete(partner)
                    .expect("minimum inner handle must be live");
                debug_assert_eq!(record.outer, handle);
                (key.0, (partner_record.outer, partner_key))
            }
        };
        let entry = self.remove_outer(handle);
        self.make_free(partner.0, partner.1);
        self.len -= 1;
        Ok((key, entry.value))
    }

    /// Returns the number of live entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns whether this heap contains no entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Removes all entries and invalidates every outstanding handle.
    pub fn clear(&mut self) {
        if !self.active {
            return;
        }
        self.min_heap.clear();
        self.max_heap.clear();
        for arena in self.arenas.values_mut() {
            arena.clear();
        }
        self.free = None;
        self.len = 0;
    }

    fn insert_outer(&mut self, key: K, value: V) -> ReflectedHandle {
        let arena = self
            .arenas
            .get_mut(&self.own_domain)
            .expect("own reflected arena must be present");
        let (slot, generation) = arena.insert(OuterEntry {
            key: Some(key),
            value,
            location: None,
        });
        ReflectedHandle {
            domain: self.own_domain,
            slot,
            generation,
        }
    }

    fn validate(&self, handle: ReflectedHandle) -> Result<ReflectedHandle, InvalidHandle> {
        if !self.active {
            return Err(InvalidHandle::ForeignHeap);
        }
        let Some(arena) = self.arenas.get(&handle.domain) else {
            return Err(InvalidHandle::ForeignHeap);
        };
        let Some(slot) = arena.slots.get(handle.slot) else {
            return Err(InvalidHandle::Stale);
        };
        if slot.generation != handle.generation || slot.entry.is_none() {
            return Err(InvalidHandle::Stale);
        }
        Ok(handle)
    }

    fn outer(&self, handle: ReflectedHandle) -> &OuterEntry<K, V> {
        self.arenas
            .get(&handle.domain)
            .and_then(|arena| arena.slots.get(handle.slot))
            .and_then(|slot| slot.entry.as_ref())
            .expect("reflected entry must be live")
    }

    fn outer_mut(&mut self, handle: ReflectedHandle) -> &mut OuterEntry<K, V> {
        self.arenas
            .get_mut(&handle.domain)
            .and_then(|arena| arena.slots.get_mut(handle.slot))
            .and_then(|slot| slot.entry.as_mut())
            .expect("reflected entry must be live")
    }

    fn remove_outer(&mut self, handle: ReflectedHandle) -> OuterEntry<K, V> {
        self.arenas
            .get_mut(&handle.domain)
            .expect("reflected arena must be present")
            .remove(handle.slot)
    }

    fn minimum_handle(&self) -> Option<ReflectedHandle> {
        match self.free {
            None => self.min_heap.peek().map(|(_, _, record)| record.outer),
            Some(free) => match self.min_heap.peek() {
                None => Some(free),
                Some((_, key, record)) => {
                    if key < self.free_key(free) {
                        Some(record.outer)
                    } else {
                        Some(free)
                    }
                }
            },
        }
    }

    fn maximum_handle(&self) -> Option<ReflectedHandle> {
        match self.free {
            None => self.max_heap.peek().map(|(_, _, record)| record.outer),
            Some(free) => match self.max_heap.peek() {
                None => Some(free),
                Some((_, key, record)) => {
                    if key.0 > *self.free_key(free) {
                        Some(record.outer)
                    } else {
                        Some(free)
                    }
                }
            },
        }
    }

    fn free_key(&self, handle: ReflectedHandle) -> &K {
        self.outer(handle)
            .key
            .as_ref()
            .expect("free entry must own its key")
    }

    fn insert_pair(&mut self, first: ReflectedHandle, second: ReflectedHandle) {
        debug_assert!(self.outer(first).location.is_none());
        debug_assert!(self.outer(second).location.is_none());
        let first_is_min = self.free_key(first) <= self.free_key(second);
        let (minimum, maximum) = if first_is_min {
            (first, second)
        } else {
            (second, first)
        };
        let minimum_key = self
            .outer_mut(minimum)
            .key
            .take()
            .expect("free entry must own its key");
        let maximum_key = self
            .outer_mut(maximum)
            .key
            .take()
            .expect("free entry must own its key");
        let minimum_inner = self.min_heap.insert(
            minimum_key,
            InnerRecord {
                outer: minimum,
                other: None,
            },
        );
        let maximum_inner = self.max_heap.insert(
            Reverse(maximum_key),
            InnerRecord {
                outer: maximum,
                other: Some(minimum_inner),
            },
        );
        *self
            .min_heap
            .value_mut(minimum_inner)
            .expect("new minimum inner handle must be live") = InnerRecord {
            outer: minimum,
            other: Some(maximum_inner),
        };
        self.outer_mut(minimum).location = Some(Location::Min(minimum_inner));
        self.outer_mut(maximum).location = Some(Location::Max(maximum_inner));
    }

    fn make_free(&mut self, handle: ReflectedHandle, key: K) {
        {
            let entry = self.outer_mut(handle);
            entry.key = Some(key);
            entry.location = None;
        }
        if let Some(free) = self.free.take() {
            self.insert_pair(handle, free);
        } else {
            self.free = Some(handle);
        }
    }

    fn repair_changed_maximum(&mut self, handle: ReflectedHandle, inner: TreeHandle, key: K) {
        let (_, record) = self
            .max_heap
            .delete(inner)
            .expect("maximum inner handle must be live");
        let partner = record.other.expect("paired inner handle must be set");
        let (partner_key, partner_record) = self
            .min_heap
            .delete(partner)
            .expect("minimum inner handle must be live");
        debug_assert_eq!(record.outer, handle);
        self.set_unpaired_key(handle, key);
        self.set_unpaired_key(partner_record.outer, partner_key);
        self.insert_pair(handle, partner_record.outer);
    }

    fn repair_changed_minimum(&mut self, handle: ReflectedHandle, inner: TreeHandle, key: K) {
        let (_, record) = self
            .min_heap
            .delete(inner)
            .expect("minimum inner handle must be live");
        let partner = record.other.expect("paired inner handle must be set");
        let (partner_key, partner_record) = self
            .max_heap
            .delete(partner)
            .expect("maximum inner handle must be live");
        debug_assert_eq!(record.outer, handle);
        self.set_unpaired_key(handle, key);
        self.set_unpaired_key(partner_record.outer, partner_key.0);
        self.insert_pair(handle, partner_record.outer);
    }

    fn set_unpaired_key(&mut self, handle: ReflectedHandle, key: K) {
        let entry = self.outer_mut(handle);
        entry.key = Some(key);
        entry.location = None;
    }
}

impl<K: Ord, V, B> Default for ReflectedHeap<K, V, B>
where
    B: ReflectedHeapBackend<K>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V, B> ReflectedHeap<K, V, B>
where
    B: ReflectedHeapBackend<K>,
    B::Min: MeldableAddressableHeap<K, InnerRecord, Handle = TreeHandle, MeldError = MeldError>,
    B::Max: MeldableAddressableHeap<Reverse<K>, InnerRecord, Handle = TreeHandle, MeldError = MeldError>,
{
    /// Melds `other` into this heap, consuming the donor on success.
    pub fn meld(&mut self, other: &mut Self) -> Result<(), MeldError> {
        if !self.active {
            return Err(MeldError::ReceiverConsumed);
        }
        if !other.active {
            return Err(MeldError::DonorConsumed);
        }
        self.min_heap.meld(&mut other.min_heap)?;
        self.max_heap.meld(&mut other.max_heap)?;
        self.arenas.extend(other.arenas.drain());
        match (self.free.take(), other.free.take()) {
            (Some(first), Some(second)) => self.insert_pair(first, second),
            (Some(free), None) | (None, Some(free)) => self.free = Some(free),
            (None, None) => {}
        }
        self.len += other.len;
        other.len = 0;
        other.active = false;
        Ok(())
    }
}

impl<K: Ord, B> ReflectedHeap<K, (), B>
where
    B: ReflectedHeapBackend<K>,
{
    /// Inserts a key into this value-less heap.
    pub fn push(&mut self, key: K) {
        self.insert(key, ());
    }

    /// Returns a minimum key, if present.
    #[must_use]
    pub fn peek(&self) -> Option<&K> {
        self.peek_entry().map(|(_, key, _)| key)
    }

    /// Removes and returns a minimum key, if present.
    pub fn pop(&mut self) -> Option<K> {
        self.pop_entry().map(|(key, ())| key)
    }

    /// Returns a maximum key, if present.
    #[must_use]
    pub fn peek_max(&self) -> Option<&K> {
        self.peek_max_entry().map(|(_, key, _)| key)
    }

    /// Removes and returns a maximum key, if present.
    pub fn pop_max(&mut self) -> Option<K> {
        self.pop_max_entry().map(|(key, ())| key)
    }
}

/// Defines a public, non-generic reflected heap type wrapping
/// [`ReflectedHeap`] with a fixed backend, forwarding its entire API.
///
/// A concrete wrapper (rather than a `pub type` alias, or exposing
/// [`ReflectedHeap`] and [`ReflectedHeapBackend`] directly) keeps the backend
/// parameter and its associated marker types out of the public API and out of
/// generated documentation.
macro_rules! define_reflected_heap {
    ($name:ident, $backend:ty, $doc:literal) => {
        #[doc = $doc]
        pub struct $name<K, V = ()>
        where
            K: Ord,
        {
            inner: ReflectedHeap<K, V, $backend>,
        }

        impl<K: Ord, V> $name<K, V> {
            /// Creates an empty heap.
            #[must_use]
            pub fn new() -> Self {
                Self {
                    inner: ReflectedHeap::new(),
                }
            }

            /// Inserts an entry unless this heap was consumed as a meld donor.
            pub fn try_insert(&mut self, key: K, value: V) -> Result<ReflectedHandle, MeldError> {
                self.inner.try_insert(key, value)
            }

            /// Inserts an entry and returns a checked handle.
            pub fn insert(&mut self, key: K, value: V) -> ReflectedHandle {
                self.inner.insert(key, value)
            }

            /// Returns the handle, key, and value of a minimum entry.
            #[must_use]
            pub fn peek_entry(&self) -> Option<(ReflectedHandle, &K, &V)> {
                self.inner.peek_entry()
            }

            /// Returns the handle, key, and value of a maximum entry.
            #[must_use]
            pub fn peek_max_entry(&self) -> Option<(ReflectedHandle, &K, &V)> {
                self.inner.peek_max_entry()
            }

            /// Removes and returns a minimum entry.
            pub fn pop_entry(&mut self) -> Option<(K, V)> {
                self.inner.pop_entry()
            }

            /// Removes and returns a maximum entry.
            pub fn pop_max_entry(&mut self) -> Option<(K, V)> {
                self.inner.pop_max_entry()
            }

            /// Returns the key identified by `handle`.
            pub fn key(&self, handle: ReflectedHandle) -> Result<&K, InvalidHandle> {
                self.inner.key(handle)
            }

            /// Returns the value identified by `handle`.
            pub fn value(&self, handle: ReflectedHandle) -> Result<&V, InvalidHandle> {
                self.inner.value(handle)
            }

            /// Returns mutable access to the value identified by `handle`.
            pub fn value_mut(&mut self, handle: ReflectedHandle) -> Result<&mut V, InvalidHandle> {
                self.inner.value_mut(handle)
            }

            /// Decreases the key identified by `handle`.
            pub fn decrease_key(
                &mut self,
                handle: ReflectedHandle,
                key: K,
            ) -> Result<(), DecreaseKeyError> {
                self.inner.decrease_key(handle, key)
            }

            /// Increases the key identified by `handle`.
            pub fn increase_key(
                &mut self,
                handle: ReflectedHandle,
                key: K,
            ) -> Result<(), IncreaseKeyError> {
                self.inner.increase_key(handle, key)
            }

            /// Removes and returns the entry identified by `handle`.
            pub fn delete(&mut self, handle: ReflectedHandle) -> Result<(K, V), InvalidHandle> {
                self.inner.delete(handle)
            }

            /// Returns the number of live entries.
            #[must_use]
            pub fn len(&self) -> usize {
                self.inner.len()
            }

            /// Returns whether this heap contains no entries.
            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.inner.is_empty()
            }

            /// Removes all entries and invalidates every outstanding handle.
            pub fn clear(&mut self) {
                self.inner.clear();
            }

            /// Melds `other` into this heap, consuming the donor on success.
            pub fn meld(&mut self, other: &mut Self) -> Result<(), MeldError> {
                self.inner.meld(&mut other.inner)
            }
        }

        impl<K: Ord, V> Default for $name<K, V> {
            fn default() -> Self {
                Self::new()
            }
        }

        impl<K: Ord> $name<K, ()> {
            /// Inserts a key into this value-less heap.
            pub fn push(&mut self, key: K) {
                self.inner.push(key);
            }

            /// Returns a minimum key, if present.
            #[must_use]
            pub fn peek(&self) -> Option<&K> {
                self.inner.peek()
            }

            /// Removes and returns a minimum key, if present.
            pub fn pop(&mut self) -> Option<K> {
                self.inner.pop()
            }

            /// Returns a maximum key, if present.
            #[must_use]
            pub fn peek_max(&self) -> Option<&K> {
                self.inner.peek_max()
            }

            /// Removes and returns a maximum key, if present.
            pub fn pop_max(&mut self) -> Option<K> {
                self.inner.pop_max()
            }
        }

        impl<K: Ord, V> AddressableHeap<K, V> for $name<K, V> {
            type Handle = ReflectedHandle;

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

            fn decrease_key(
                &mut self,
                handle: Self::Handle,
                key: K,
            ) -> Result<(), DecreaseKeyError> {
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

        impl<K: Ord, V> DoubleEndedAddressableHeap<K, V> for $name<K, V> {
            fn peek_max(&self) -> Option<(Self::Handle, &K, &V)> {
                Self::peek_max_entry(self)
            }

            fn pop_max(&mut self) -> Option<(K, V)> {
                Self::pop_max_entry(self)
            }

            fn increase_key(
                &mut self,
                handle: Self::Handle,
                key: K,
            ) -> Result<(), IncreaseKeyError> {
                Self::increase_key(self, handle, key)
            }
        }

        impl<K: Ord, V> MeldableAddressableHeap<K, V> for $name<K, V> {
            type MeldError = MeldError;

            fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
                Self::meld(self, other)
            }
        }

        impl<K: Ord, V> MeldableDoubleEndedAddressableHeap<K, V> for $name<K, V> {
            type MeldError = MeldError;

            fn meld(&mut self, other: &mut Self) -> Result<(), Self::MeldError> {
                Self::meld(self, other)
            }
        }

        crate::impl_heap_via_addressable!($name);
        crate::impl_meldable_heap_via_addressable!($name);
        crate::impl_double_ended_heap_via_addressable!($name);
    };
}

define_reflected_heap!(
    ReflectedFibonacciHeap,
    FibonacciReflectedBackend,
    "A reflected double-ended heap built from Fibonacci heaps."
);
define_reflected_heap!(
    ReflectedPairingHeap,
    PairingReflectedBackend,
    "A reflected double-ended heap built from pairing heaps."
);
