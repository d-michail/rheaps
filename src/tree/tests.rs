//! Shared tree-heap conformance fixtures patterned after the array suite.

use core::cmp::Ordering;
use std::collections::HashMap;

use crate::error::{DecreaseKeyError, IncreaseKeyError, InvalidHandle};
use crate::test_support::ReverseKey;
use crate::{AddressableHeap, DecreaseKeyHeap, DoubleEndedAddressableHeap, DoubleEndedHeap, Heap};

use super::{
    BinaryTreeAddressableHeap, BinaryTreeSoftAddressableHeap, BinaryTreeSoftHeap,
    CostlessMeldPairingHeap, DaryTreeAddressableHeap, FibonacciHeap, LeftistHeap, PairingHeap,
    PurePairingHeap, RankPairingHeap, ReflectedFibonacciHeap, ReflectedPairingHeap,
    SimpleFibonacciHeap, SkewHeap, SoftHeapError, SoftMeldError, StrictFibonacciHeap, TreeHandle,
};

const STRESS_SIZE: i32 = 2_000;

struct Random(u64);

impl Random {
    fn next_i32(&mut self) -> i32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        (self.0 >> 32) as i32
    }
}

fn exercise_heap<H>(make: impl Fn() -> H)
where
    H: Heap<i32>,
{
    let mut heap = make();
    assert!(heap.peek().is_none());
    assert!(heap.pop().is_none());
    for value in (0..STRESS_SIZE).rev() {
        heap.push(value);
    }
    for offset in 0..STRESS_SIZE {
        let expected = offset;
        assert_eq!(heap.peek(), Some(&expected));
        assert_eq!(heap.pop(), Some(expected));
    }

    let mut random = Random(1);
    let mut heap = make();
    for _ in 0..STRESS_SIZE {
        heap.push(random.next_i32());
    }
    let mut previous = None;
    while let Some(value) = heap.pop() {
        if let Some(previous) = previous {
            assert!(previous <= value, "tree heap output was not ordered");
        }
        previous = Some(value);
    }
}

fn exercise_reverse_tree_heap<H>(make: impl Fn() -> H)
where
    H: Heap<ReverseKey>,
{
    let mut heap = make();
    for value in 0..STRESS_SIZE {
        heap.push(ReverseKey(value));
    }
    for expected in (0..STRESS_SIZE).rev() {
        assert_eq!(heap.peek(), Some(&ReverseKey(expected)));
        assert_eq!(heap.pop(), Some(ReverseKey(expected)));
    }

    let mut random = Random(1);
    let values = (0..STRESS_SIZE)
        .map(|_| random.next_i32())
        .collect::<Vec<_>>();
    let mut expected = values.clone();
    expected.sort_unstable_by(|left, right| right.cmp(left));
    let mut heap = make();
    for value in values {
        heap.push(ReverseKey(value));
    }
    for value in expected {
        assert_eq!(heap.pop(), Some(ReverseKey(value)));
    }
}

fn exercise_reverse_tree_addressable_heap<H>(make: impl Fn() -> H)
where
    H: AddressableHeap<ReverseKey, usize> + DecreaseKeyHeap<ReverseKey, usize>,
    H::Handle: Copy + Eq,
{
    let mut heap = make();
    let handles = (0..256)
        .map(|key| heap.insert(ReverseKey(key), key as usize))
        .collect::<Vec<_>>();
    assert_eq!(heap.peek().map(|(_, key, _)| *key), Some(ReverseKey(255)));
    heap.decrease_key(handles[0], ReverseKey(256)).unwrap();
    assert_eq!(heap.pop(), Some((ReverseKey(256), 0)));
    assert_eq!(heap.key(handles[0]), Err(InvalidHandle::Stale));

    let mut previous = i32::MAX;
    while let Some((key, value)) = heap.pop() {
        assert_eq!(key.0 as usize, value);
        assert!(previous >= key.0);
        previous = key.0;
    }
    let stale = heap.insert(ReverseKey(1), 1);
    heap.clear();
    assert_eq!(heap.key(stale), Err(InvalidHandle::Stale));
}

fn exercise_reverse_addressable_random_operations<H>(make: impl Fn() -> H)
where
    H: AddressableHeap<ReverseKey, usize> + DecreaseKeyHeap<ReverseKey, usize>,
    H::Handle: Copy + Eq,
{
    let mut heap = make();
    let mut random = Random(3);
    let mut entries = Vec::<Option<(i32, H::Handle)>>::new();
    for value in 0..STRESS_SIZE as usize {
        let key = random.next_i32();
        entries.push(Some((key, heap.insert(ReverseKey(key), value))));
    }
    for _ in 0..STRESS_SIZE {
        let index = (random.next_i32() as u32 as usize) % entries.len();
        if let Some((key, handle)) = entries[index] {
            match random.next_i32() & 3 {
                0 => {
                    let next = key.saturating_add((random.next_i32() as u32 & 255) as i32);
                    heap.decrease_key(handle, ReverseKey(next)).unwrap();
                    entries[index] = Some((next, handle));
                }
                1 => {
                    assert_eq!(heap.delete(handle), Ok((ReverseKey(key), index)));
                    entries[index] = None;
                }
                _ => {
                    let (popped, value) = heap.pop().unwrap();
                    let expected = entries
                        .iter()
                        .filter_map(|entry| entry.map(|(key, _)| key))
                        .max()
                        .unwrap();
                    assert_eq!(popped, ReverseKey(expected));
                    assert_eq!(entries[value].map(|(key, _)| key), Some(expected));
                    entries[value] = None;
                }
            }
        }
    }
    let mut previous = i32::MAX;
    while let Some((ReverseKey(key), value)) = heap.pop() {
        assert_eq!(entries[value].map(|(expected, _)| expected), Some(key));
        entries[value] = None;
        assert!(previous >= key);
        previous = key;
    }
    assert!(entries.iter().all(Option::is_none));
}

fn exercise_reverse_double_ended_addressable_heap<H>(make: impl Fn() -> H)
where
    H: AddressableHeap<ReverseKey, usize>
        + DoubleEndedAddressableHeap<ReverseKey, usize>
        + DecreaseKeyHeap<ReverseKey, usize>,
    H::Handle: Copy + Eq,
{
    let mut heap = make();
    let first = heap.insert(ReverseKey(3), 0);
    let second = heap.insert(ReverseKey(1), 1);
    let third = heap.insert(ReverseKey(5), 2);
    assert_eq!(heap.peek().map(|(_, key, _)| key.0), Some(5));
    assert_eq!(heap.peek_max().map(|(_, key, _)| key.0), Some(1));
    heap.decrease_key(first, ReverseKey(6)).unwrap();
    heap.increase_key(second, ReverseKey(0)).unwrap();
    assert_eq!(
        heap.increase_key(third, ReverseKey(6)),
        Err(IncreaseKeyError::NotIncreased)
    );
    assert_eq!(
        heap.decrease_key(third, ReverseKey(4)),
        Err(DecreaseKeyError::NotDecreased)
    );
    assert_eq!(heap.delete(first), Ok((ReverseKey(6), 0)));

    let mut entries = vec![None, Some((0, second)), Some((5, third))];
    let mut random = Random(9);
    for value in 3..STRESS_SIZE as usize {
        let key = random.next_i32();
        entries.push(Some((key, heap.insert(ReverseKey(key), value))));
    }
    for _ in 0..STRESS_SIZE {
        let index = (random.next_i32() as u32 as usize) % entries.len();
        if let Some((key, handle)) = entries[index] {
            match random.next_i32() & 3 {
                0 => {
                    let next = key.saturating_add((random.next_i32() as u32 & 255) as i32);
                    heap.decrease_key(handle, ReverseKey(next)).unwrap();
                    entries[index] = Some((next, handle));
                }
                1 => {
                    let next = key.saturating_sub((random.next_i32() as u32 & 255) as i32);
                    heap.increase_key(handle, ReverseKey(next)).unwrap();
                    entries[index] = Some((next, handle));
                }
                2 => {
                    let expected = entries
                        .iter()
                        .filter_map(|entry| entry.map(|e| e.0))
                        .max()
                        .unwrap();
                    let (ReverseKey(key), value) = heap.pop().unwrap();
                    assert_eq!(key, expected);
                    entries[value] = None;
                }
                _ => {
                    let expected = entries
                        .iter()
                        .filter_map(|entry| entry.map(|e| e.0))
                        .min()
                        .unwrap();
                    let (ReverseKey(key), value) = heap.pop_max().unwrap();
                    assert_eq!(key, expected);
                    entries[value] = None;
                }
            }
        }
    }
    while let Some((ReverseKey(key), value)) = heap.pop() {
        let expected = entries
            .iter()
            .filter_map(|entry| entry.map(|e| e.0))
            .max()
            .unwrap();
        assert_eq!(key, expected);
        entries[value] = None;
    }
    assert!(entries.iter().all(Option::is_none));
}

fn exercise_addressable_heap<H>(make: impl Fn() -> H)
where
    H: AddressableHeap<i32, usize> + DecreaseKeyHeap<i32, usize>,
    H::Handle: Copy + Eq,
{
    let mut heap = make();
    assert!(heap.peek().is_none());
    assert!(heap.pop().is_none());

    let handles = (0..256)
        .map(|key| heap.insert(key, key as usize))
        .collect::<Vec<_>>();
    for key in [17, 5, 255, 0, 73, 109, 1, 200] {
        assert_eq!(heap.delete(handles[key]), Ok((key as i32, key)));
        assert_eq!(heap.key(handles[key]), Err(InvalidHandle::Stale));
    }

    let handle = heap.insert(1_000, 1_000);
    let decreased = -1;
    heap.decrease_key(handle, decreased).unwrap();
    assert_eq!(heap.key(handle), Ok(&decreased));
    let invalid = 2_000;
    assert_eq!(
        heap.decrease_key(handle, invalid),
        Err(DecreaseKeyError::NotDecreased)
    );
    *heap.value_mut(handle).unwrap() = 7;
    assert_eq!(heap.value(handle), Ok(&7));

    let mut previous = None;
    while let Some((key, _)) = heap.pop() {
        if let Some(previous) = previous {
            assert!(previous <= key);
        }
        previous = Some(key);
    }
    assert_eq!(heap.key(handle), Err(InvalidHandle::Stale));

    let stale = heap.insert(4, 4);
    heap.clear();
    assert_eq!(heap.key(stale), Err(InvalidHandle::Stale));
    assert_eq!(heap.pop(), None);
    let reused = heap.insert(3, 3);
    assert_eq!(heap.pop(), Some((3, 3)));
    assert_eq!(heap.key(reused), Err(InvalidHandle::Stale));

    let mut foreign = make();
    let foreign_handle = foreign.insert(5, 5);
    assert_eq!(heap.key(foreign_handle), Err(InvalidHandle::ForeignHeap));
    assert_eq!(
        heap.value_mut(foreign_handle),
        Err(InvalidHandle::ForeignHeap)
    );
    assert_eq!(
        heap.decrease_key(foreign_handle, 4),
        Err(DecreaseKeyError::InvalidHandle(InvalidHandle::ForeignHeap))
    );
}

fn exercise_addressable_random_operations<H>(make: impl Fn() -> H)
where
    H: AddressableHeap<i32, usize> + DecreaseKeyHeap<i32, usize>,
    H::Handle: Copy + Eq,
{
    let mut heap = make();
    let mut random = Random(3);
    let mut entries = Vec::<Option<(i32, H::Handle)>>::new();

    for value in 0..STRESS_SIZE as usize {
        let key = random.next_i32();
        entries.push(Some((key, heap.insert(key, value))));
    }

    for _ in 0..STRESS_SIZE {
        let index = (random.next_i32() as u32 as usize) % entries.len();
        if let Some((key, handle)) = entries[index] {
            match random.next_i32() & 3 {
                0 => {
                    let next = key.saturating_sub((random.next_i32() as u32 & 255) as i32);
                    heap.decrease_key(handle, next).unwrap();
                    entries[index] = Some((next, handle));
                }
                1 => {
                    assert_eq!(heap.delete(handle), Ok((key, index)));
                    entries[index] = None;
                }
                _ => {
                    let (popped_key, value) = heap.pop().unwrap();
                    let expected = entries
                        .iter()
                        .filter_map(|entry| entry.map(|(key, _)| key))
                        .min()
                        .unwrap();
                    assert_eq!(popped_key, expected);
                    assert_eq!(entries[value].map(|(key, _)| key), Some(popped_key));
                    entries[value] = None;
                }
            }
        }
    }

    let mut previous = None;
    while let Some((key, value)) = heap.pop() {
        assert_eq!(entries[value].map(|(expected, _)| expected), Some(key));
        entries[value] = None;
        if let Some(previous) = previous {
            assert!(previous <= key);
        }
        previous = Some(key);
    }
    assert!(entries.iter().all(Option::is_none));
}

fn exercise_binary_addressable_heap<H>(make: impl Fn() -> H + Copy)
where
    H: AddressableHeap<i32, usize> + DecreaseKeyHeap<i32, usize>,
    H::Handle: Copy + Eq,
{
    exercise_addressable_heap(make);
    let mut random = Random(2);
    let mut heap = make();
    let mut keys = Vec::new();
    let mut handles = Vec::new();
    for value in 0..STRESS_SIZE {
        let key = random.next_i32();
        keys.push(key);
        handles.push(heap.insert(key, value as usize));
    }
    for index in (0..STRESS_SIZE as usize).step_by(7) {
        let next = keys[index].saturating_sub(10);
        heap.decrease_key(handles[index], next).unwrap();
    }
    let mut previous = None;
    while let Some((key, _)) = heap.pop() {
        if let Some(previous) = previous {
            assert!(previous <= key);
        }
        previous = Some(key);
    }
}

fn expected_extreme(entries: &[Option<(i32, impl Copy)>], maximum: bool) -> Option<i32> {
    entries
        .iter()
        .filter_map(|entry| entry.as_ref().map(|(key, _)| *key))
        .reduce(|left, right| {
            let order = left.cmp(&right);
            if (maximum && order == Ordering::Less) || (!maximum && order == Ordering::Greater) {
                right
            } else {
                left
            }
        })
}

struct TrackedStrictHeap {
    heap: StrictFibonacciHeap<i32, usize>,
    live: HashMap<usize, (i32, TreeHandle)>,
}

impl TrackedStrictHeap {
    fn new() -> Self {
        Self {
            heap: StrictFibonacciHeap::new(),
            live: HashMap::new(),
        }
    }

    fn assert_valid(&self) {
        self.heap.assert_invariants();
        assert_eq!(self.heap.len(), self.live.len());
        let expected = self.live.values().map(|(key, _)| *key).min();
        assert_eq!(self.heap.peek().map(|(_, key, _)| *key), expected);
        for (&value, &(key, handle)) in &self.live {
            assert_eq!(self.heap.key(handle), Ok(&key));
            assert_eq!(self.heap.value(handle), Ok(&value));
        }
    }
}

fn exercise_strict_fibonacci_random_operations(operation_count: usize, seed: u64, key_bound: i32) {
    let mut random = Random(seed);
    let mut heaps = Vec::<TrackedStrictHeap>::new();
    let mut next_value = 0_usize;

    for _ in 0..operation_count {
        let choice = random.next_i32() as u32 % 100;
        if heaps.is_empty() || choice < 5 {
            heaps.push(TrackedStrictHeap::new());
        } else if choice < 10 && heaps.len() >= 2 {
            let first_index = random.next_i32() as u32 as usize % heaps.len();
            let mut first = heaps.swap_remove(first_index);
            let second_index = random.next_i32() as u32 as usize % heaps.len();
            let second = heaps.swap_remove(second_index);
            first.heap.meld(second.heap);
            first.live.extend(second.live);
            first.assert_valid();
            heaps.push(first);
        } else {
            let heap_index = random.next_i32() as u32 as usize % heaps.len();
            let tracked = &mut heaps[heap_index];
            if choice < 50 && !tracked.live.is_empty() {
                let entry_index = random.next_i32() as u32 as usize % tracked.live.len();
                let (&value, &(old_key, handle)) = tracked.live.iter().nth(entry_index).unwrap();
                let decrease = 1 + (random.next_i32() as u32 % 25) as i32;
                let key = old_key.saturating_sub(decrease);
                tracked.heap.decrease_key(handle, key).unwrap();
                tracked.live.insert(value, (key, handle));
            } else if choice < 80 {
                let key = (random.next_i32() as u32 % key_bound as u32) as i32;
                let handle = tracked.heap.insert(key, next_value);
                tracked.live.insert(next_value, (key, handle));
                next_value += 1;
            } else if !tracked.live.is_empty() {
                let expected = tracked.live.values().map(|(key, _)| *key).min().unwrap();
                let (key, value) = tracked.heap.pop().unwrap();
                assert_eq!(key, expected);
                assert_eq!(tracked.live.remove(&value).map(|entry| entry.0), Some(key));
            }
        }

        for tracked in &heaps {
            tracked.assert_valid();
        }
    }
}

fn exercise_double_ended_addressable_heap<H>(make: impl Fn() -> H)
where
    H: AddressableHeap<i32, usize>
        + DoubleEndedAddressableHeap<i32, usize>
        + DecreaseKeyHeap<i32, usize>,
    H::Handle: Copy + Eq,
{
    let mut heap = make();
    assert!(heap.peek().is_none());
    assert!(heap.peek_max().is_none());
    assert!(heap.pop().is_none());
    assert!(heap.pop_max().is_none());

    let first = heap.insert(3, 0);
    let second = heap.insert(1, 1);
    let third = heap.insert(5, 2);
    assert_eq!(heap.peek().map(|(_, key, _)| *key), Some(1));
    assert_eq!(heap.peek_max().map(|(_, key, _)| *key), Some(5));
    let decreased = 0;
    heap.decrease_key(first, decreased).unwrap();
    let increased = 7;
    heap.increase_key(second, increased).unwrap();
    assert_eq!(
        heap.increase_key(third, 4),
        Err(IncreaseKeyError::NotIncreased)
    );
    assert_eq!(
        heap.decrease_key(third, 6),
        Err(DecreaseKeyError::NotDecreased)
    );
    assert_eq!(heap.delete(first), Ok((decreased, 0)));
    assert_eq!(heap.key(first), Err(InvalidHandle::Stale));

    let mut foreign = make();
    let foreign_handle = foreign.insert(10, 10);
    assert_eq!(
        heap.increase_key(foreign_handle, 11),
        Err(IncreaseKeyError::InvalidHandle(InvalidHandle::ForeignHeap))
    );

    let mut entries = vec![None, Some((increased, second)), Some((5, third))];
    let mut random = Random(9);
    for value in 3..STRESS_SIZE as usize {
        let key = random.next_i32();
        entries.push(Some((key, heap.insert(key, value))));
    }

    for _ in 0..STRESS_SIZE {
        let index = (random.next_i32() as u32 as usize) % entries.len();
        if let Some((key, handle)) = entries[index] {
            match random.next_i32() & 3 {
                0 => {
                    let next = key.saturating_sub((random.next_i32() as u32 & 255) as i32);
                    heap.decrease_key(handle, next).unwrap();
                    entries[index] = Some((next, handle));
                }
                1 => {
                    let next = key.saturating_add((random.next_i32() as u32 & 255) as i32);
                    heap.increase_key(handle, next).unwrap();
                    entries[index] = Some((next, handle));
                }
                2 => {
                    let expected = expected_extreme(&entries, false).unwrap();
                    let (popped, value) = heap.pop().unwrap();
                    assert_eq!(popped, expected);
                    assert_eq!(entries[value].map(|(entry, _)| entry), Some(popped));
                    entries[value] = None;
                }
                _ => {
                    let expected = expected_extreme(&entries, true).unwrap();
                    let (popped, value) = heap.pop_max().unwrap();
                    assert_eq!(popped, expected);
                    assert_eq!(entries[value].map(|(entry, _)| entry), Some(popped));
                    entries[value] = None;
                }
            }
        }
    }

    while !heap.is_empty() {
        let expected = expected_extreme(&entries, false).unwrap();
        let (key, value) = heap.pop().unwrap();
        assert_eq!(key, expected);
        entries[value] = None;
    }
    assert!(entries.iter().all(Option::is_none));
}

macro_rules! exercise_meld {
    ($heap:ident) => {{
        let mut first = $heap::<i32, usize>::new();
        let mut second = $heap::<i32, usize>::new();
        for value in 0..100 {
            first.insert(value * 2, value as usize);
        }
        let donor_handle = second.insert(201, 201);
        for value in 0..100 {
            second.insert(value * 2 + 1, value as usize);
        }
        first.meld(second);
        first.decrease_key(donor_handle, -1).unwrap();
        assert_eq!(first.peek().map(|(_, key, _)| *key), Some(-1));
        assert_eq!(first.delete(donor_handle), Ok((-1, 201)));
        assert_eq!(first.key(donor_handle), Err(InvalidHandle::Stale));
        let mut expected = 0;
        while let Some((key, _)) = first.pop() {
            assert!(expected <= key);
            expected = key;
        }

        let mut a = $heap::<i32, usize>::new();
        let mut b = $heap::<i32, usize>::new();
        let mut c = $heap::<i32, usize>::new();
        let handle = c.insert(10, 10);
        b.meld(c);
        a.meld(b);
        a.decrease_key(handle, 0).unwrap();
        assert_eq!(a.peek().map(|(_, key, _)| *key), Some(0));

        let mut empty_receiver = $heap::<i32, usize>::new();
        let empty_donor = $heap::<i32, usize>::new();
        empty_receiver.meld(empty_donor);
        assert!(empty_receiver.is_empty());

        for (receiver_len, donor_len) in [(100, 100), (101, 101), (101, 100)] {
            let mut receiver = $heap::<i32, usize>::new();
            let mut donor = $heap::<i32, usize>::new();
            for value in 0..receiver_len {
                receiver.insert(value * 2, value as usize);
            }
            for value in 0..donor_len {
                donor.insert(value * 2 + 1, value as usize);
            }
            receiver.meld(donor);
            let mut previous = None;
            let mut count = 0;
            while let Some((key, _)) = receiver.pop() {
                if let Some(previous) = previous {
                    assert!(previous <= key);
                }
                previous = Some(key);
                count += 1;
            }
            assert_eq!(count, receiver_len + donor_len);
        }

        let mut reusable = $heap::<i32, usize>::new();
        let mut reusable_donor = $heap::<i32, usize>::new();
        reusable_donor.insert(1, 1);
        reusable.meld(reusable_donor);
        reusable.insert(0, 0);
        assert_eq!(reusable.pop(), Some((0, 0)));
        reusable.clear();
        reusable.insert(2, 2);
        assert_eq!(reusable.pop(), Some((2, 2)));
    }};
}

#[test]
fn dary_tree_addressable_heap_enforces_power_of_two_branching() {
    assert!(DaryTreeAddressableHeap::<i32, ()>::new(0).is_err());
    assert!(DaryTreeAddressableHeap::<i32, ()>::new(3).is_err());
    assert!(DaryTreeAddressableHeap::<i32, ()>::new(2).is_ok());
}

#[test]
fn soft_heaps_validate_error_rates_and_preserve_every_key() {
    assert!(matches!(
        BinaryTreeSoftHeap::<i32>::new(0.0),
        Err(SoftHeapError::NonPositiveErrorRate)
    ));
    assert!(matches!(
        BinaryTreeSoftHeap::<i32>::new(-0.5),
        Err(SoftHeapError::NonPositiveErrorRate)
    ));
    assert!(matches!(
        BinaryTreeSoftHeap::<i32>::new(1.0),
        Err(SoftHeapError::ErrorRateNotBelowOne)
    ));
    assert!(matches!(
        BinaryTreeSoftHeap::<i32>::new(f64::NAN),
        Err(SoftHeapError::ErrorRateNotBelowOne)
    ));

    let mut heap = BinaryTreeSoftHeap::new(0.5).unwrap();
    for key in (0..STRESS_SIZE).rev() {
        heap.push(key);
    }
    let mut keys = Vec::new();
    while let Some(key) = heap.pop() {
        keys.push(key);
    }
    keys.sort_unstable();
    assert_eq!(keys, (0..STRESS_SIZE).collect::<Vec<_>>());

    let mut receiver = BinaryTreeSoftHeap::new(0.5).unwrap();
    let mut donor = BinaryTreeSoftHeap::new(0.5).unwrap();
    receiver.push(2);
    donor.push(1);
    receiver.meld(donor).unwrap();
    assert_eq!(receiver.pop(), Some(1));
    assert_eq!(receiver.pop(), Some(2));

    let mut alternate = BinaryTreeSoftHeap::new(0.5).unwrap();
    for key in 0..32 {
        alternate.push(ReverseKey(key));
    }
    for expected in (0..32).rev() {
        assert_eq!(alternate.pop(), Some(ReverseKey(expected)));
    }

    for error_rate in [0.01, 0.25, 0.5, 0.75, 0.99] {
        let mut heap = BinaryTreeSoftHeap::new(error_rate).unwrap();
        for key in 0..STRESS_SIZE {
            heap.push(key);
        }
        let mut removed = Vec::new();
        for _ in 0..STRESS_SIZE / 4 {
            removed.push(heap.pop().unwrap());
        }
        while let Some(key) = heap.pop() {
            removed.push(key);
        }
        removed.sort_unstable();
        assert_eq!(removed, (0..STRESS_SIZE).collect::<Vec<_>>());

        let mut small = BinaryTreeSoftHeap::new(error_rate).unwrap();
        let mut large = BinaryTreeSoftHeap::new(error_rate).unwrap();
        for key in 0..STRESS_SIZE / 3 {
            small.push(key);
        }
        for key in STRESS_SIZE / 3..STRESS_SIZE {
            large.push(key);
        }
        small.meld(large).unwrap();
        assert_eq!(small.len(), STRESS_SIZE as usize);
        let mut melded = Vec::new();
        while let Some(key) = small.pop() {
            melded.push(key);
        }
        melded.sort_unstable();
        assert_eq!(melded, (0..STRESS_SIZE).collect::<Vec<_>>());
    }
}

#[test]
fn soft_addressable_heaps_enforce_handles_and_meld_rules() {
    let mut heap = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    let first = heap.insert(3, 3);
    let second = heap.insert(1, 1);
    let third = heap.insert(2, 2);
    *heap.value_mut(second).unwrap() = 10;
    assert_eq!(heap.value(second), Ok(&10));
    // `BinaryTreeSoftAddressableHeap` does not implement `DecreaseKeyHeap`;
    // key decreases are rejected at compile time rather than at run time.
    assert_eq!(heap.delete(third), Ok((2, 2)));
    assert_eq!(heap.key(third), Err(InvalidHandle::Stale));
    assert_eq!(heap.value_mut(third), Err(InvalidHandle::Stale));
    let mut seen = vec![heap.delete(first).unwrap().0, heap.pop_entry().unwrap().0];
    seen.sort_unstable();
    assert_eq!(seen, vec![1, 3]);

    let mut receiver = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    let mut donor = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    let donor_handle = donor.insert(4, 4);
    receiver.insert(5, 5);
    receiver.meld(donor).unwrap();
    assert_eq!(receiver.key(donor_handle), Ok(&4));
    assert_eq!(receiver.delete(donor_handle), Ok((4, 4)));

    let incompatible = BinaryTreeSoftAddressableHeap::new(0.01).unwrap();
    assert_eq!(
        receiver.meld(incompatible),
        Err(SoftMeldError::IncompatibleErrorRate)
    );
    let mut stress = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    let mut random = Random(11);
    let mut entries = Vec::new();
    for value in 0..STRESS_SIZE as usize {
        let key = random.next_i32();
        entries.push(Some((key, stress.insert(key, value))));
    }
    for _ in 0..STRESS_SIZE {
        let index = (random.next_i32() as u32 as usize) % entries.len();
        if let Some((key, handle)) = entries[index] {
            if random.next_i32() & 1 == 0 {
                assert_eq!(stress.delete(handle), Ok((key, index)));
                assert_eq!(stress.key(handle), Err(InvalidHandle::Stale));
                entries[index] = None;
            } else {
                let (popped_key, value) = stress.pop_entry().unwrap();
                assert_eq!(entries[value].map(|(entry, _)| entry), Some(popped_key));
                entries[value] = None;
            }
        }
    }
    while let Some((key, value)) = stress.pop_entry() {
        assert_eq!(entries[value].map(|(entry, _)| entry), Some(key));
        entries[value] = None;
    }
    assert!(entries.iter().all(Option::is_none));
    let stale = receiver.insert(10, 10);
    receiver.clear();
    assert_eq!(receiver.key(stale), Err(InvalidHandle::Stale));

    let mut alternate = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    for key in 0..32 {
        alternate.insert(ReverseKey(key), key as usize);
    }
    for expected in (0..32).rev() {
        assert_eq!(
            alternate.pop_entry(),
            Some((ReverseKey(expected), expected as usize))
        );
    }

    for error_rate in [0.01, 0.25, 0.5, 0.75, 0.99] {
        let mut heap = BinaryTreeSoftAddressableHeap::new(error_rate).unwrap();
        let mut handles = Vec::new();
        for key in 0..STRESS_SIZE {
            handles.push(heap.insert(key, key));
        }
        for index in (0..handles.len()).step_by(4) {
            assert_eq!(
                heap.delete(handles[index]),
                Ok((index as i32, index as i32))
            );
        }
        let mut remaining = Vec::new();
        while let Some((key, value)) = heap.pop_entry() {
            assert_eq!(key, value);
            remaining.push(key);
        }
        remaining.sort_unstable();
        assert_eq!(
            remaining,
            (0..STRESS_SIZE)
                .filter(|key| key % 4 != 0)
                .collect::<Vec<_>>()
        );
    }

    let mut a = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    let mut b = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    let mut c = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    let mut d = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    let mut e = BinaryTreeSoftAddressableHeap::new(0.5).unwrap();
    let a_handle = a.insert(12, 12);
    let b_handle = b.insert(16, 16);
    let c_handle = c.insert(20, 20);
    let d_handle = d.insert(24, 24);
    let e_handle = e.insert(28, 28);
    e.insert(29, 29);
    d.meld(e).unwrap();
    c.meld(d).unwrap();
    b.meld(c).unwrap();
    a.meld(b).unwrap();
    for (handle, key) in [
        (a_handle, 12),
        (b_handle, 16),
        (c_handle, 20),
        (d_handle, 24),
        (e_handle, 28),
    ] {
        assert_eq!(a.key(handle), Ok(&key));
        assert_eq!(a.delete(handle), Ok((key, key)));
    }
    assert_eq!(a.pop_entry(), Some((29, 29)));
    assert!(a.is_empty());
}

#[test]
fn leftist_heap_follows_common_heap_conformance() {
    exercise_heap(LeftistHeap::<i32>::new);
}

#[test]
fn skew_heap_follows_common_heap_conformance() {
    exercise_heap(SkewHeap::<i32>::new);
}

#[test]
fn pairing_heap_follows_common_heap_conformance() {
    exercise_heap(PairingHeap::<i32>::new);
}

#[test]
fn fibonacci_heap_follows_common_heap_conformance() {
    exercise_heap(FibonacciHeap::<i32>::new);
}

#[test]
fn simple_fibonacci_heap_follows_common_heap_conformance() {
    exercise_heap(SimpleFibonacciHeap::<i32>::new);
}

#[test]
fn pure_pairing_heap_follows_common_heap_conformance() {
    exercise_heap(PurePairingHeap::<i32>::new);
}

#[test]
fn rank_pairing_heap_follows_common_heap_conformance() {
    exercise_heap(RankPairingHeap::<i32>::new);
}

#[test]
fn costless_meld_pairing_heap_follows_common_heap_conformance() {
    exercise_heap(CostlessMeldPairingHeap::<i32>::new);
}

#[test]
fn strict_fibonacci_heap_follows_common_heap_conformance() {
    exercise_heap(StrictFibonacciHeap::<i32>::new);
}

#[test]
fn reflected_fibonacci_heap_follows_common_heap_conformance() {
    exercise_heap(ReflectedFibonacciHeap::<i32>::new);
}

#[test]
fn reflected_pairing_heap_follows_common_heap_conformance() {
    exercise_heap(ReflectedPairingHeap::<i32>::new);
}

#[test]
fn binary_tree_addressable_heap_gains_value_less_heap_trait() {
    exercise_heap(BinaryTreeAddressableHeap::<i32>::new);
}

#[test]
fn dary_tree_addressable_heap_gains_value_less_heap_trait() {
    exercise_heap(|| DaryTreeAddressableHeap::<i32>::new(4).unwrap());
}

#[test]
fn binary_tree_soft_addressable_heap_gains_value_less_push_peek_pop() {
    fn accepts_heap<H: Heap<i32>>(heap: &mut H) {
        heap.push(-1);
        assert_eq!(heap.peek(), Some(&-1));
    }

    let mut heap = BinaryTreeSoftAddressableHeap::<i32, ()>::new(0.5).unwrap();
    heap.push(5);
    heap.push(3);
    heap.push(8);
    assert_eq!(heap.len(), 3);
    let mut seen = Vec::new();
    while let Some(key) = heap.pop() {
        seen.push(key);
    }
    seen.sort_unstable();
    assert_eq!(seen, vec![3, 5, 8]);

    accepts_heap(&mut BinaryTreeSoftAddressableHeap::<i32, ()>::new(0.5).unwrap());
}

#[test]
fn alternate_ord_keys_preserve_leftist_heap_ordering() {
    exercise_reverse_tree_heap(LeftistHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_skew_heap_ordering() {
    exercise_reverse_tree_heap(SkewHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_pairing_heap_ordering() {
    exercise_reverse_tree_heap(PairingHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_fibonacci_heap_ordering() {
    exercise_reverse_tree_heap(FibonacciHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_simple_fibonacci_heap_ordering() {
    exercise_reverse_tree_heap(SimpleFibonacciHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_pure_pairing_heap_ordering() {
    exercise_reverse_tree_heap(PurePairingHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_rank_pairing_heap_ordering() {
    exercise_reverse_tree_heap(RankPairingHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_costless_meld_pairing_heap_ordering() {
    exercise_reverse_tree_heap(CostlessMeldPairingHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_strict_fibonacci_heap_ordering() {
    exercise_reverse_tree_heap(StrictFibonacciHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_reflected_fibonacci_heap_ordering() {
    exercise_reverse_tree_heap(ReflectedFibonacciHeap::<ReverseKey>::new);
}

#[test]
fn alternate_ord_keys_preserve_reflected_pairing_heap_ordering() {
    exercise_reverse_tree_heap(ReflectedPairingHeap::<ReverseKey>::new);
}

#[test]
fn leftist_heap_enforces_handle_rules() {
    exercise_addressable_heap(LeftistHeap::<i32, usize>::new);
    exercise_addressable_random_operations(LeftistHeap::<i32, usize>::new);
}

#[test]
fn skew_heap_enforces_handle_rules() {
    exercise_addressable_heap(SkewHeap::<i32, usize>::new);
    exercise_addressable_random_operations(SkewHeap::<i32, usize>::new);
}

#[test]
fn pairing_heap_enforces_handle_rules() {
    exercise_addressable_heap(PairingHeap::<i32, usize>::new);
    exercise_addressable_random_operations(PairingHeap::<i32, usize>::new);
}

#[test]
fn fibonacci_heap_enforces_handle_rules() {
    exercise_addressable_heap(FibonacciHeap::<i32, usize>::new);
    exercise_addressable_random_operations(FibonacciHeap::<i32, usize>::new);
}

#[test]
fn simple_fibonacci_heap_enforces_handle_rules() {
    exercise_addressable_heap(SimpleFibonacciHeap::<i32, usize>::new);
    exercise_addressable_random_operations(SimpleFibonacciHeap::<i32, usize>::new);
}

#[test]
fn pure_pairing_heap_enforces_handle_rules() {
    exercise_addressable_heap(PurePairingHeap::<i32, usize>::new);
    exercise_addressable_random_operations(PurePairingHeap::<i32, usize>::new);
}

#[test]
fn rank_pairing_heap_enforces_handle_rules() {
    exercise_addressable_heap(RankPairingHeap::<i32, usize>::new);
    exercise_addressable_random_operations(RankPairingHeap::<i32, usize>::new);
}

#[test]
fn costless_meld_pairing_heap_enforces_handle_rules() {
    exercise_addressable_heap(CostlessMeldPairingHeap::<i32, usize>::new);
    exercise_addressable_random_operations(CostlessMeldPairingHeap::<i32, usize>::new);
}

#[test]
fn strict_fibonacci_heap_enforces_handle_rules() {
    exercise_addressable_heap(StrictFibonacciHeap::<i32, usize>::new);
    exercise_addressable_random_operations(StrictFibonacciHeap::<i32, usize>::new);
}

#[test]
fn binary_tree_addressable_heap_enforces_handle_rules() {
    exercise_binary_addressable_heap(BinaryTreeAddressableHeap::<i32, usize>::new);
    exercise_addressable_random_operations(BinaryTreeAddressableHeap::<i32, usize>::new);
}

#[test]
fn dary_tree_addressable_heap_degree_2_enforces_handle_rules() {
    exercise_binary_addressable_heap(|| DaryTreeAddressableHeap::<i32, usize>::new(2).unwrap());
    exercise_addressable_random_operations(|| {
        DaryTreeAddressableHeap::<i32, usize>::new(2).unwrap()
    });
}

#[test]
fn dary_tree_addressable_heap_degree_4_enforces_handle_rules() {
    exercise_binary_addressable_heap(|| DaryTreeAddressableHeap::<i32, usize>::new(4).unwrap());
    exercise_addressable_random_operations(|| {
        DaryTreeAddressableHeap::<i32, usize>::new(4).unwrap()
    });
}

#[test]
fn dary_tree_addressable_heap_degree_8_enforces_handle_rules() {
    exercise_binary_addressable_heap(|| DaryTreeAddressableHeap::<i32, usize>::new(8).unwrap());
    exercise_addressable_random_operations(|| {
        DaryTreeAddressableHeap::<i32, usize>::new(8).unwrap()
    });
}

#[test]
fn dary_tree_addressable_heap_degree_16_enforces_handle_rules() {
    exercise_binary_addressable_heap(|| DaryTreeAddressableHeap::<i32, usize>::new(16).unwrap());
    exercise_addressable_random_operations(|| {
        DaryTreeAddressableHeap::<i32, usize>::new(16).unwrap()
    });
}

#[test]
fn reflected_fibonacci_heap_enforces_handle_rules() {
    exercise_addressable_heap(ReflectedFibonacciHeap::<i32, usize>::new);
    exercise_addressable_random_operations(ReflectedFibonacciHeap::<i32, usize>::new);
}

#[test]
fn reflected_pairing_heap_enforces_handle_rules() {
    exercise_addressable_heap(ReflectedPairingHeap::<i32, usize>::new);
    exercise_addressable_random_operations(ReflectedPairingHeap::<i32, usize>::new);
}

#[test]
fn alternate_ord_keys_preserve_leftist_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(LeftistHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(LeftistHeap::<ReverseKey, usize>::new);
}

#[test]
fn alternate_ord_keys_preserve_skew_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(SkewHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(SkewHeap::<ReverseKey, usize>::new);
}

#[test]
fn alternate_ord_keys_preserve_pairing_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(PairingHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(PairingHeap::<ReverseKey, usize>::new);
}

#[test]
fn alternate_ord_keys_preserve_fibonacci_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(FibonacciHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(FibonacciHeap::<ReverseKey, usize>::new);
}

#[test]
fn alternate_ord_keys_preserve_simple_fibonacci_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(SimpleFibonacciHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(SimpleFibonacciHeap::<ReverseKey, usize>::new);
}

#[test]
fn alternate_ord_keys_preserve_pure_pairing_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(PurePairingHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(PurePairingHeap::<ReverseKey, usize>::new);
}

#[test]
fn alternate_ord_keys_preserve_rank_pairing_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(RankPairingHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(RankPairingHeap::<ReverseKey, usize>::new);
}

#[test]
fn alternate_ord_keys_preserve_costless_meld_pairing_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(CostlessMeldPairingHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(
        CostlessMeldPairingHeap::<ReverseKey, usize>::new,
    );
}

#[test]
fn alternate_ord_keys_preserve_strict_fibonacci_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(StrictFibonacciHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(StrictFibonacciHeap::<ReverseKey, usize>::new);
}

#[test]
fn alternate_ord_keys_preserve_binary_tree_addressable_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(BinaryTreeAddressableHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(
        BinaryTreeAddressableHeap::<ReverseKey, usize>::new,
    );
}

#[test]
fn alternate_ord_keys_preserve_dary_tree_addressable_heap_degree_2_handle_ordering() {
    exercise_reverse_tree_addressable_heap(|| {
        DaryTreeAddressableHeap::<ReverseKey, usize>::new(2).unwrap()
    });
    exercise_reverse_addressable_random_operations(|| {
        DaryTreeAddressableHeap::<ReverseKey, usize>::new(2).unwrap()
    });
}

#[test]
fn alternate_ord_keys_preserve_dary_tree_addressable_heap_degree_4_handle_ordering() {
    exercise_reverse_tree_addressable_heap(|| {
        DaryTreeAddressableHeap::<ReverseKey, usize>::new(4).unwrap()
    });
    exercise_reverse_addressable_random_operations(|| {
        DaryTreeAddressableHeap::<ReverseKey, usize>::new(4).unwrap()
    });
}

#[test]
fn alternate_ord_keys_preserve_dary_tree_addressable_heap_degree_8_handle_ordering() {
    exercise_reverse_tree_addressable_heap(|| {
        DaryTreeAddressableHeap::<ReverseKey, usize>::new(8).unwrap()
    });
    exercise_reverse_addressable_random_operations(|| {
        DaryTreeAddressableHeap::<ReverseKey, usize>::new(8).unwrap()
    });
}

#[test]
fn alternate_ord_keys_preserve_dary_tree_addressable_heap_degree_16_handle_ordering() {
    exercise_reverse_tree_addressable_heap(|| {
        DaryTreeAddressableHeap::<ReverseKey, usize>::new(16).unwrap()
    });
    exercise_reverse_addressable_random_operations(|| {
        DaryTreeAddressableHeap::<ReverseKey, usize>::new(16).unwrap()
    });
}

#[test]
fn alternate_ord_keys_preserve_reflected_fibonacci_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(ReflectedFibonacciHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(
        ReflectedFibonacciHeap::<ReverseKey, usize>::new,
    );
}

#[test]
fn alternate_ord_keys_preserve_reflected_pairing_heap_handle_ordering() {
    exercise_reverse_tree_addressable_heap(ReflectedPairingHeap::<ReverseKey, usize>::new);
    exercise_reverse_addressable_random_operations(ReflectedPairingHeap::<ReverseKey, usize>::new);
}

#[test]
fn reflected_fibonacci_heap_maintains_both_extrema_through_key_changes() {
    exercise_double_ended_addressable_heap(ReflectedFibonacciHeap::<i32, usize>::new);

    let mut heap = ReflectedFibonacciHeap::<i32>::new();
    Heap::push(&mut heap, 3);
    Heap::push(&mut heap, 1);
    Heap::push(&mut heap, 5);
    assert_eq!(DoubleEndedHeap::peek_max(&heap), Some(&5));
    assert_eq!(DoubleEndedHeap::pop_max(&mut heap), Some(5));
    assert_eq!(Heap::pop(&mut heap), Some(1));
}

#[test]
fn reflected_pairing_heap_maintains_both_extrema_through_key_changes() {
    exercise_double_ended_addressable_heap(ReflectedPairingHeap::<i32, usize>::new);

    let mut heap = ReflectedPairingHeap::<i32>::new();
    Heap::push(&mut heap, 3);
    Heap::push(&mut heap, 1);
    Heap::push(&mut heap, 5);
    assert_eq!(DoubleEndedHeap::peek_max(&heap), Some(&5));
    assert_eq!(DoubleEndedHeap::pop_max(&mut heap), Some(5));
    assert_eq!(Heap::pop(&mut heap), Some(1));
}

#[test]
fn reflected_fibonacci_heap_applies_ord_to_both_extrema() {
    exercise_reverse_double_ended_addressable_heap(
        ReflectedFibonacciHeap::<ReverseKey, usize>::new,
    );
    let mut fibonacci = ReflectedFibonacciHeap::<ReverseKey>::new();
    Heap::push(&mut fibonacci, ReverseKey(3));
    Heap::push(&mut fibonacci, ReverseKey(1));
    Heap::push(&mut fibonacci, ReverseKey(5));
    assert_eq!(Heap::peek(&fibonacci), Some(&ReverseKey(5)));
    assert_eq!(DoubleEndedHeap::peek_max(&fibonacci), Some(&ReverseKey(1)));
}

#[test]
fn reflected_pairing_heap_applies_ord_to_both_extrema() {
    exercise_reverse_double_ended_addressable_heap(ReflectedPairingHeap::<ReverseKey, usize>::new);
    let mut pairing = ReflectedPairingHeap::<ReverseKey>::new();
    Heap::push(&mut pairing, ReverseKey(3));
    Heap::push(&mut pairing, ReverseKey(1));
    Heap::push(&mut pairing, ReverseKey(5));
    assert_eq!(Heap::peek(&pairing), Some(&ReverseKey(5)));
    assert_eq!(DoubleEndedHeap::peek_max(&pairing), Some(&ReverseKey(1)));
}

#[test]
fn leftist_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(LeftistHeap);
}

#[test]
fn skew_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(SkewHeap);
}

#[test]
fn pairing_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(PairingHeap);
}

#[test]
fn fibonacci_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(FibonacciHeap);
}

#[test]
fn simple_fibonacci_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(SimpleFibonacciHeap);
}

#[test]
fn pure_pairing_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(PurePairingHeap);
}

#[test]
fn rank_pairing_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(RankPairingHeap);
}

#[test]
fn costless_meld_pairing_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(CostlessMeldPairingHeap);
}

#[test]
fn strict_fibonacci_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(StrictFibonacciHeap);
}

#[test]
fn reflected_fibonacci_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(ReflectedFibonacciHeap);
}

#[test]
fn reflected_pairing_heap_meld_consumes_donor_and_preserves_handles() {
    exercise_meld!(ReflectedPairingHeap);
}

macro_rules! exercise_invariants {
    ($heap:ident) => {{
        let mut heap = $heap::<i32, usize>::new();
        let handles = (0..128)
            .rev()
            .map(|key| heap.insert(key, key as usize))
            .collect::<Vec<_>>();
        heap.assert_invariants();

        for index in (0..handles.len()).step_by(3) {
            heap.decrease_key(handles[index], -1_000 - index as i32)
                .unwrap();
            heap.assert_invariants();
        }
        for index in (1..handles.len()).step_by(4) {
            heap.delete(handles[index]).unwrap();
            heap.assert_invariants();
        }

        let mut donor = $heap::<i32, usize>::new();
        let donor_handle = donor.insert(-2_000, 999);
        heap.meld(donor);
        heap.assert_invariants();
        heap.decrease_key(donor_handle, -3_000).unwrap();
        heap.assert_invariants();

        let mut previous = None;
        while let Some((key, _)) = heap.pop() {
            if let Some(previous) = previous {
                assert!(previous <= key);
            }
            previous = Some(key);
            heap.assert_invariants();
        }
    }};
}

#[test]
fn fibonacci_heap_maintains_node_forest_invariants() {
    exercise_invariants!(FibonacciHeap);
}

#[test]
fn simple_fibonacci_heap_maintains_node_forest_invariants() {
    exercise_invariants!(SimpleFibonacciHeap);
}

#[test]
fn pure_pairing_heap_maintains_node_forest_invariants() {
    exercise_invariants!(PurePairingHeap);
}

#[test]
fn rank_pairing_heap_maintains_node_forest_invariants() {
    exercise_invariants!(RankPairingHeap);
}

#[test]
fn costless_meld_pairing_heap_maintains_node_forest_invariants() {
    exercise_invariants!(CostlessMeldPairingHeap);
}

#[test]
fn strict_fibonacci_heap_maintains_node_forest_invariants() {
    exercise_invariants!(StrictFibonacciHeap);
}

#[test]
fn strict_fibonacci_random_operations_small() {
    exercise_strict_fibonacci_random_operations(2_000, 1, 50);
}

#[test]
fn strict_fibonacci_random_operations_medium() {
    exercise_strict_fibonacci_random_operations(4_000, 2, 300);
}

#[test]
fn strict_fibonacci_random_operations_large() {
    exercise_strict_fibonacci_random_operations(20_000, 3, 5_000);
}

#[test]
fn strict_fibonacci_random_operations_many_duplicate_keys() {
    exercise_strict_fibonacci_random_operations(20_000, 4, 5);
}

#[cfg(feature = "serde")]
#[test]
fn pairing_heap_round_trips_through_serde_json() {
    let mut heap = PairingHeap::<i32>::new();
    heap.push(3);
    heap.push(1);
    heap.push(2);

    let json = serde_json::to_string(&heap).unwrap();
    let mut restored: PairingHeap<i32> = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.pop(), Some(1));
    assert_eq!(restored.pop(), Some(2));
    assert_eq!(restored.pop(), Some(3));
    assert_eq!(restored.pop(), None);
}

#[cfg(feature = "serde")]
#[test]
fn fibonacci_heap_round_trips_through_serde_json() {
    let mut heap = FibonacciHeap::<i32>::new();
    heap.push(3);
    heap.push(1);
    heap.push(2);

    let json = serde_json::to_string(&heap).unwrap();
    let mut restored: FibonacciHeap<i32> = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.pop(), Some(1));
    assert_eq!(restored.pop(), Some(2));
    assert_eq!(restored.pop(), Some(3));
    assert_eq!(restored.pop(), None);
}

#[cfg(feature = "serde")]
#[test]
fn binary_tree_addressable_heap_round_trips_through_serde_json() {
    let mut heap = BinaryTreeAddressableHeap::new();
    let task = heap.insert(4, "clean up");
    heap.insert(1, "reply to mail");

    let json = serde_json::to_string(&heap).unwrap();
    let mut restored: BinaryTreeAddressableHeap<i32, &str> = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.key(task), Ok(&4));
    assert_eq!(restored.value(task), Ok(&"clean up"));
    assert_eq!(restored.pop_entry(), Some((1, "reply to mail")));
    assert_eq!(restored.pop_entry(), Some((4, "clean up")));
}

#[cfg(feature = "serde")]
#[test]
fn reflected_fibonacci_heap_round_trips_through_serde_json() {
    let mut heap = ReflectedFibonacciHeap::<i32>::new();
    Heap::push(&mut heap, 4);
    Heap::push(&mut heap, 1);
    Heap::push(&mut heap, 3);

    let json = serde_json::to_string(&heap).unwrap();
    let mut restored: ReflectedFibonacciHeap<i32> = serde_json::from_str(&json).unwrap();

    assert_eq!(Heap::peek(&restored), Some(&1));
    assert_eq!(DoubleEndedHeap::peek_max(&restored), Some(&4));
    assert_eq!(DoubleEndedHeap::pop_max(&mut restored), Some(4));
    assert_eq!(Heap::pop(&mut restored), Some(1));
}
