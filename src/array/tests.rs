//! Reusable conformance fixtures translated from JHeaps' array-package JUnit
//! suite. Generic helpers exercise [`Heap`], [`AddressableHeap`], and
//! [`DoubleEndedHeap`] implementations; concrete checks remain only for
//! capabilities outside those traits. Rust's `Option` and ownership model
//! replace Java tests for null keys and empty-heap exceptions.

use core::cmp::Ordering;
use std::collections::HashSet;

use super::{
    AddressableHandle, BinaryArrayAddressableHeap, BinaryArrayBulkInsertWeakHeap, BinaryArrayHeap,
    BinaryArrayIntegerValueHeap, BinaryArrayWeakHeap, DaryArrayAddressableHeap, DaryArrayHeap,
    DecreaseKeyError, InvalidDegree, InvalidHandle, MinMaxBinaryArrayDoubleEndedHeap,
};
use crate::test_support::ReverseKey;
use crate::{AddressableHeap, DoubleEndedHeap, Heap, ValueHeap};

const STRESS_SIZE: i32 = 10_000;

struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MASK: u64 = (1 << 48) - 1;
    const MULTIPLIER: u64 = 0x5DEECE66D;
    const ADDEND: u64 = 0xB;

    fn new(seed: u64) -> Self {
        Self {
            seed: (seed ^ Self::MULTIPLIER) & Self::MASK,
        }
    }

    fn next(&mut self, bits: u32) -> u32 {
        self.seed = (self
            .seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::ADDEND))
            & Self::MASK;
        (self.seed >> (48 - bits)) as u32
    }

    fn next_i32(&mut self) -> i32 {
        self.next(32) as i32
    }

    fn next_i32_bound(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if bound & (bound - 1) == 0 {
            return ((i64::from(bound) * i64::from(self.next(31))) >> 31) as i32;
        }
        loop {
            let bits = self.next(31) as i32;
            let value = bits % bound;
            if bits - value <= i32::MAX - (bound - 1) {
                return value;
            }
        }
    }
}

fn drain<H>(mut heap: H) -> Vec<i32>
where
    H: Heap<i32>,
{
    core::iter::from_fn(|| heap.pop()).collect()
}

fn assert_ordered_by(values: &[i32], order: Ordering) {
    for pair in values.windows(2) {
        assert_ne!(pair[0].cmp(&pair[1]), order);
    }
}

fn exercise_min_heap<H>(make: impl Fn() -> H)
where
    H: Heap<i32>,
{
    let mut heap = make();
    assert!(heap.is_empty());
    assert_eq!(heap.len(), 0);
    assert_eq!(heap.peek(), None);
    assert_eq!(heap.pop(), None);

    for value in 0..STRESS_SIZE {
        heap.push(value);
        assert_eq!(heap.len(), (value + 1) as usize);
        assert_eq!(heap.peek(), Some(&0));
    }
    for offset in 0..STRESS_SIZE {
        let expected = offset;
        assert_eq!(heap.peek(), Some(&expected));
        assert_eq!(heap.pop(), Some(expected));
    }
    assert!(heap.is_empty());

    let mut heap = make();
    for value in (0..STRESS_SIZE).rev() {
        heap.push(value);
        assert_eq!(heap.peek(), Some(&value));
    }
    assert_eq!(heap.len(), STRESS_SIZE as usize);
    heap.clear();
    assert!(heap.is_empty());
    heap.push(780);
    heap.push(-389);
    assert_eq!(heap.pop(), Some(-389));

    let mut heap = make();
    for value in [780, -389, 306, 579] {
        heap.push(value);
    }
    let expected = vec![-389, 306, 579, 780];
    assert_eq!(drain(heap), expected);

    for seed in [1, 2] {
        let mut random = JavaRandom::new(seed);
        let mut heap = make();
        for _ in 0..STRESS_SIZE {
            heap.push(random.next_i32());
        }
        let mut result = Vec::with_capacity(STRESS_SIZE as usize);
        while let Some(peeked) = heap.peek().copied() {
            assert_eq!(heap.pop(), Some(peeked));
            result.push(peeked);
        }
        assert_ordered_by(&result, Ordering::Greater);
    }

    let mut random = JavaRandom::new(1);
    let mut heap = make();
    for _ in 0..STRESS_SIZE {
        heap.push(random.next_i32_bound(1_000) - 500);
    }
    let result = drain(heap);
    assert_eq!(result.len(), STRESS_SIZE as usize);
    assert_ordered_by(&result, Ordering::Greater);
}

fn exercise_double_ended_heap<H>(make: impl Fn() -> H)
where
    H: DoubleEndedHeap<i32>,
{
    let mut heap = make();
    assert_eq!(heap.peek_max(), None);
    assert_eq!(heap.pop_max(), None);

    for value in 0..STRESS_SIZE {
        heap.push(value);
        assert_eq!(heap.peek(), Some(&0));
        assert_eq!(heap.peek_max(), Some(&value));
    }
    for offset in 0..STRESS_SIZE {
        let expected = STRESS_SIZE - offset - 1;
        assert_eq!(heap.pop_max(), Some(expected));
        if offset + 1 < STRESS_SIZE {
            assert_eq!(heap.peek(), Some(&0));
        }
    }
    assert!(heap.is_empty());

    for seed in 1..=6 {
        let mut random = JavaRandom::new(seed);
        let mut heap = make();
        for _ in 0..STRESS_SIZE {
            heap.push(random.next_i32());
        }
        let mut result = Vec::with_capacity(STRESS_SIZE as usize);
        while let Some(peeked) = heap.peek_max().copied() {
            assert_eq!(heap.pop_max(), Some(peeked));
            result.push(peeked);
        }
        assert_ordered_by(&result, Ordering::Less);
    }

    for seed in 13..1_000 {
        let mut random = JavaRandom::new(seed);
        let mut heap = make();
        for _ in 0..STRESS_SIZE / 100 {
            heap.push(random.next_i32());
        }
        let mut result = Vec::with_capacity((STRESS_SIZE / 100) as usize);
        while let Some(value) = heap.pop_max() {
            result.push(value);
        }
        assert_ordered_by(&result, Ordering::Less);
    }

    let mut heap = make();
    for value in [900, 800, 780, 850] {
        heap.push(value);
    }
    assert_eq!(heap.peek(), Some(&780));
    assert_eq!(heap.peek_max(), Some(&900));
    assert_eq!(heap.pop_max(), Some(900));
}

fn addressable_min<H>(heap: &H) -> Option<i32>
where
    H: AddressableHeap<i32, usize, Handle = AddressableHandle>,
{
    heap.peek().map(|(_, key, _)| *key)
}

fn exercise_addressable_heap<H>(make: impl Fn() -> H)
where
    H: AddressableHeap<i32, usize, Handle = AddressableHandle>,
{
    let mut heap = make();
    assert!(heap.is_empty());
    assert_eq!(heap.pop(), None);
    assert_eq!(heap.peek(), None);

    let mut handles = Vec::with_capacity(STRESS_SIZE as usize);
    for key in 0..STRESS_SIZE {
        handles.push(heap.push(key, key as usize));
        assert_eq!(addressable_min(&heap), Some(0));
        assert_eq!(heap.len(), (key + 1) as usize);
    }
    for offset in 0..STRESS_SIZE {
        let key = offset;
        assert_eq!(heap.key(handles[key as usize]), Ok(&key));
        assert_eq!(heap.pop(), Some((key, key as usize)));
    }
    assert!(heap.is_empty());

    let mut heap = make();
    for key in (0..STRESS_SIZE).rev() {
        heap.push(key, key as usize);
        assert_eq!(addressable_min(&heap), Some(key));
    }
    heap.clear();
    assert!(heap.is_empty());
    let handle = heap.push(1, 1);
    *heap.value_mut(handle).unwrap() = 2;
    assert_eq!(heap.value(handle), Ok(&2));
    heap.decrease_key(handle, 1).unwrap();
    assert_eq!(heap.pop(), Some((1, 2)));
    assert_eq!(heap.key(handle), Err(InvalidHandle::Stale));
    assert_eq!(heap.value_mut(handle), Err(InvalidHandle::Stale));

    let mut heap = make();
    let handles = (0..128)
        .map(|key| heap.push(key, key as usize))
        .collect::<Vec<_>>();
    let mut live = vec![true; handles.len()];
    for index in [5, 7, 0, 2, 1, 3, 9, 4, 8, 11, 6, 12, 10, 13, 14] {
        assert_eq!(heap.delete(handles[index]), Ok((index as i32, index)));
        live[index] = false;
        let expected = live
            .iter()
            .position(|&is_live| is_live)
            .map(|index| index as i32);
        assert_eq!(addressable_min(&heap), expected);
    }

    let mut heap = make();
    let handles = (0..STRESS_SIZE)
        .map(|key| heap.push(key, key as usize))
        .collect::<Vec<_>>();
    for key in (0..STRESS_SIZE).rev() {
        assert_eq!(heap.delete(handles[key as usize]), Ok((key, key as usize)));
        if key != 0 {
            assert_eq!(addressable_min(&heap), Some(0));
        }
    }
    assert!(heap.is_empty());

    for seed in [1, 2] {
        let mut random = JavaRandom::new(seed);
        let mut heap = make();
        for _ in 0..STRESS_SIZE {
            let key = random.next_i32();
            heap.push(key, key as usize);
        }
        let mut previous = None;
        while let Some(peeked) = addressable_min(&heap) {
            let (key, value) = heap.pop().unwrap();
            assert_eq!(key, peeked);
            assert_eq!(key as usize, value);
            if let Some(previous) = previous {
                assert!(previous <= key, "entries are not ordered");
            }
            previous = Some(key);
        }
    }

    for (seed, bound, offset) in [(1, 1_000, 0), (3, 2_001, -1_000)] {
        let mut random = JavaRandom::new(seed);
        let mut heap = make();
        for _ in 0..STRESS_SIZE {
            let key = random.next_i32_bound(bound) + offset;
            heap.push(key, key as usize);
        }
        let mut result = Vec::with_capacity(STRESS_SIZE as usize);
        while let Some((key, value)) = heap.pop() {
            assert_eq!(key as usize, value);
            result.push(key);
        }
        assert_eq!(result.len(), STRESS_SIZE as usize);
        assert_ordered_by(&result, Ordering::Greater);
    }

    let mut heap = make();
    let mut current = Vec::with_capacity(STRESS_SIZE as usize);
    let mut handles = Vec::with_capacity(STRESS_SIZE as usize);
    for index in 0..STRESS_SIZE {
        let key = 2 * index;
        current.push(key);
        handles.push(heap.push(key, index as usize));
    }
    let mut random = JavaRandom::new(1);
    for _ in 0..STRESS_SIZE / 2 {
        let index = random.next_i32_bound(STRESS_SIZE) as usize;
        let old_key = current[index];
        let new_key = if old_key > 0 {
            random.next_i32_bound(old_key)
        } else {
            0
        };
        heap.decrease_key(handles[index], new_key).unwrap();
        current[index] = new_key;
    }
    let mut result = Vec::with_capacity(STRESS_SIZE as usize);
    while let Some((key, _)) = heap.pop() {
        result.push(key);
    }
    assert_eq!(result.len(), STRESS_SIZE as usize);
    assert_ordered_by(&result, Ordering::Greater);
}

fn exercise_integer_value_heap() {
    let mut heap = BinaryArrayIntegerValueHeap::with_capacity(0);
    assert!(heap.is_empty());
    assert_eq!(heap.peek(), None);
    assert_eq!(heap.pop(), None);

    for key in 0..STRESS_SIZE {
        heap.push(key, key);
        assert_eq!(heap.peek(), Some((&0, &0)));
    }
    for key in 0..STRESS_SIZE {
        assert_eq!(heap.peek_key(), Some(&key));
        assert_eq!(heap.peek_value(), Some(&key));
        assert_eq!(heap.pop(), Some((key, key)));
    }

    let mut heap = BinaryArrayIntegerValueHeap::new();
    for key in [i32::MAX, 5, i32::MIN, -1, i32::MIN, 5] {
        heap.push(key, key);
    }
    assert_eq!(
        core::iter::from_fn(|| heap.pop()).collect::<Vec<_>>(),
        vec![
            (i32::MIN, i32::MIN),
            (i32::MIN, i32::MIN),
            (-1, -1),
            (5, 5),
            (5, 5),
            (i32::MAX, i32::MAX),
        ]
    );

    let mut heap = BinaryArrayIntegerValueHeap::from_vec(vec![(4, 40), (-2, -20), (1, 10)]);
    assert_eq!(heap.peek_key(), Some(&-2));
    assert_eq!(heap.peek_value(), Some(&-20));
    assert_eq!(heap.pop(), Some((-2, -20)));
    assert_eq!(heap.pop(), Some((1, 10)));
    assert_eq!(heap.pop(), Some((4, 40)));

    for seed in [1, 2] {
        let mut random = JavaRandom::new(seed);
        let mut heap = BinaryArrayIntegerValueHeap::new();
        for _ in 0..STRESS_SIZE {
            let key = random.next_i32();
            heap.push(key, key);
        }
        let mut previous = None;
        while let Some((key, value)) = heap.pop() {
            assert_eq!(key, value);
            if let Some(previous) = previous {
                assert!(previous <= key);
            }
            previous = Some(key);
        }
        heap.clear();
        assert!(heap.is_empty());
    }

    fn accepts_value_heap<H: ValueHeap<i32, i32>>(mut heap: H) {
        heap.push(2, 2);
        heap.push(1, 1);
        assert_eq!(heap.peek(), Some((&1, &1)));
        assert_eq!(heap.pop(), Some((1, 1)));
    }
    accepts_value_heap(BinaryArrayIntegerValueHeap::new());
}

fn assert_constructed_empty<H>(mut heap: H)
where
    H: Heap<i32>,
{
    assert!(heap.is_empty());
    heap.push(1);
    assert_eq!(heap.peek(), Some(&1));
    assert_eq!(heap.pop(), Some(1));
}

fn assert_addressable_construction<H>(mut heap: H)
where
    H: AddressableHeap<i32, i32, Handle = AddressableHandle>,
{
    let mut previous = None;
    while let Some((key, value)) = heap.pop() {
        assert_eq!(key, value);
        if let Some(previous) = previous {
            assert!(previous <= key);
        }
        previous = Some(key);
    }
}

fn assert_empty_addressable_is_reusable<H>(mut heap: H)
where
    H: AddressableHeap<i32, i32, Handle = AddressableHandle>,
{
    assert!(heap.is_empty());
    let handle = heap.push(1, 1);
    assert_eq!(heap.key(handle), Ok(&1));
    assert_eq!(heap.pop(), Some((1, 1)));
}

fn assert_dary_handles_are_live(heap: DaryArrayAddressableHeap<i32, i32>) {
    let handles = {
        let mut iterator = heap.handles();
        let handles = iterator.by_ref().collect::<Vec<_>>();
        assert_eq!(iterator.next(), None);
        handles
    };
    let keys = handles
        .into_iter()
        .map(|handle| *heap.key(handle).unwrap())
        .collect::<HashSet<_>>();
    assert_eq!(keys.len(), STRESS_SIZE as usize);
    assert_eq!(heap.handles().count(), STRESS_SIZE as usize);
}

fn drain_reverse<H>(mut heap: H) -> Vec<i32>
where
    H: Heap<ReverseKey>,
{
    core::iter::from_fn(|| heap.pop().map(|key| key.0)).collect()
}

fn exercise_reverse_min_heap<H>(make: impl Fn() -> H)
where
    H: Heap<ReverseKey>,
{
    let mut heap = make();
    assert!(heap.is_empty());
    assert_eq!(heap.peek(), None);
    assert_eq!(heap.pop(), None);
    for value in 0..STRESS_SIZE {
        heap.push(ReverseKey(value));
        assert_eq!(heap.peek(), Some(&ReverseKey(value)));
    }
    for value in (0..STRESS_SIZE).rev() {
        assert_eq!(heap.pop(), Some(ReverseKey(value)));
    }

    for seed in [1, 2] {
        let mut random = JavaRandom::new(seed);
        let values = (0..STRESS_SIZE)
            .map(|_| random.next_i32())
            .collect::<Vec<_>>();
        let mut expected = values.clone();
        expected.sort_unstable_by(|left, right| right.cmp(left));
        let mut heap = make();
        for value in values {
            heap.push(ReverseKey(value));
        }
        assert_eq!(drain_reverse(heap), expected);
    }

    let mut random = JavaRandom::new(1);
    let mut heap = make();
    for _ in 0..STRESS_SIZE {
        heap.push(ReverseKey(random.next_i32_bound(1_000) - 500));
    }
    let values = drain_reverse(heap);
    assert_eq!(values.len(), STRESS_SIZE as usize);
    assert_ordered_by(&values, Ordering::Less);

    let mut heap = make();
    heap.push(ReverseKey(780));
    heap.push(ReverseKey(-389));
    assert_eq!(heap.pop(), Some(ReverseKey(780)));
    heap.clear();
    assert!(heap.is_empty());
}

fn exercise_reverse_double_ended_heap<H>(make: impl Fn() -> H)
where
    H: DoubleEndedHeap<ReverseKey>,
{
    let mut heap = make();
    assert_eq!(heap.peek_max(), None);
    assert_eq!(heap.pop_max(), None);
    for value in 0..STRESS_SIZE {
        heap.push(ReverseKey(value));
        assert_eq!(heap.peek(), Some(&ReverseKey(value)));
        assert_eq!(heap.peek_max(), Some(&ReverseKey(0)));
    }
    for expected in 0..STRESS_SIZE {
        assert_eq!(heap.pop_max(), Some(ReverseKey(expected)));
    }

    for seed in 1..=6 {
        let mut random = JavaRandom::new(seed);
        let mut heap = make();
        for _ in 0..STRESS_SIZE {
            heap.push(ReverseKey(random.next_i32()));
        }
        let values = core::iter::from_fn(|| heap.pop_max().map(|key| key.0)).collect::<Vec<_>>();
        assert_ordered_by(&values, Ordering::Greater);
    }
}

fn exercise_reverse_addressable_heap<H>(make: impl Fn() -> H)
where
    H: AddressableHeap<ReverseKey, usize, Handle = AddressableHandle>,
{
    let mut heap = make();
    let handles = (0..STRESS_SIZE)
        .map(|key| heap.push(ReverseKey(key), key as usize))
        .collect::<Vec<_>>();
    assert_eq!(
        heap.peek().map(|(_, key, _)| *key),
        Some(ReverseKey(STRESS_SIZE - 1))
    );
    heap.decrease_key(handles[0], ReverseKey(STRESS_SIZE))
        .unwrap();
    assert_eq!(heap.pop(), Some((ReverseKey(STRESS_SIZE), 0)));
    assert_eq!(heap.key(handles[0]), Err(InvalidHandle::Stale));

    let mut previous = None;
    while let Some((key, value)) = heap.pop() {
        assert_eq!(key.0 as usize, value);
        if let Some(previous) = previous {
            assert!(previous >= key.0);
        }
        previous = Some(key.0);
    }

    let stale = heap.push(ReverseKey(1), 1);
    heap.clear();
    assert_eq!(heap.key(stale), Err(InvalidHandle::Stale));

    for seed in [1, 2] {
        let mut random = JavaRandom::new(seed);
        let mut heap = make();
        for _ in 0..STRESS_SIZE {
            let key = random.next_i32();
            heap.push(ReverseKey(key), key as usize);
        }
        let mut previous = i32::MAX;
        while let Some((ReverseKey(key), value)) = heap.pop() {
            assert_eq!(key as usize, value);
            assert!(previous >= key, "entries are not reverse ordered");
            previous = key;
        }
    }

    let mut heap = make();
    let mut current = Vec::with_capacity(STRESS_SIZE as usize);
    let mut handles = Vec::with_capacity(STRESS_SIZE as usize);
    for index in 0..STRESS_SIZE {
        let key = 2 * index;
        current.push(key);
        handles.push(heap.push(ReverseKey(key), index as usize));
    }
    let mut random = JavaRandom::new(1);
    for _ in 0..STRESS_SIZE / 2 {
        let index = random.next_i32_bound(STRESS_SIZE) as usize;
        let old_key = current[index];
        let new_key = old_key.saturating_add(random.next_i32_bound(STRESS_SIZE));
        heap.decrease_key(handles[index], ReverseKey(new_key))
            .unwrap();
        current[index] = new_key;
    }
    let values = core::iter::from_fn(|| heap.pop().map(|(key, _)| key.0)).collect::<Vec<_>>();
    assert_eq!(values.len(), STRESS_SIZE as usize);
    assert_ordered_by(&values, Ordering::Less);
}

#[test]
fn jheaps_min_heap_behavior_all_array_implementations() {
    exercise_min_heap(|| BinaryArrayHeap::with_capacity(0));
    exercise_min_heap(|| DaryArrayHeap::with_capacity(2, 0).unwrap());
    exercise_min_heap(|| DaryArrayHeap::with_capacity(3, 0).unwrap());
    exercise_min_heap(|| DaryArrayHeap::with_capacity(4, 0).unwrap());
    exercise_min_heap(|| DaryArrayHeap::with_capacity(5, 0).unwrap());
    exercise_min_heap(|| BinaryArrayWeakHeap::with_capacity(0));
    exercise_min_heap(|| BinaryArrayBulkInsertWeakHeap::with_capacity(0));
    exercise_min_heap(|| MinMaxBinaryArrayDoubleEndedHeap::with_capacity(0));
}

#[test]
fn alternate_ord_keys_preserve_array_heap_ordering() {
    exercise_reverse_min_heap(BinaryArrayHeap::new);
    exercise_reverse_min_heap(|| DaryArrayHeap::new(2).unwrap());
    exercise_reverse_min_heap(|| DaryArrayHeap::new(3).unwrap());
    exercise_reverse_min_heap(|| DaryArrayHeap::new(4).unwrap());
    exercise_reverse_min_heap(|| DaryArrayHeap::new(5).unwrap());
    exercise_reverse_min_heap(BinaryArrayWeakHeap::new);
    exercise_reverse_min_heap(BinaryArrayBulkInsertWeakHeap::new);
    exercise_reverse_min_heap(MinMaxBinaryArrayDoubleEndedHeap::new);
    exercise_reverse_double_ended_heap(MinMaxBinaryArrayDoubleEndedHeap::new);

    let values = (0..STRESS_SIZE).map(ReverseKey).collect::<Vec<_>>();
    let expected = (0..STRESS_SIZE).rev().collect::<Vec<_>>();
    assert_eq!(
        drain_reverse(BinaryArrayHeap::from_vec(values.clone())),
        expected
    );
    assert_eq!(
        drain_reverse(DaryArrayHeap::from_vec(3, values.clone()).unwrap()),
        expected
    );
    assert_eq!(
        drain_reverse(BinaryArrayWeakHeap::from_vec(values.clone())),
        expected
    );
    assert_eq!(
        drain_reverse(BinaryArrayBulkInsertWeakHeap::from_vec(values.clone())),
        expected
    );
    assert_eq!(
        drain_reverse(MinMaxBinaryArrayDoubleEndedHeap::from_vec(values)),
        expected
    );

    let mut double_ended = MinMaxBinaryArrayDoubleEndedHeap::new();
    for value in 0..STRESS_SIZE {
        double_ended.push(ReverseKey(value));
    }
    assert_eq!(double_ended.peek(), Some(&ReverseKey(STRESS_SIZE - 1)));
    assert_eq!(double_ended.peek_max(), Some(&ReverseKey(0)));
    assert_eq!(double_ended.pop_max(), Some(ReverseKey(0)));
}

#[test]
fn jheaps_double_ended_behavior_and_random_properties() {
    exercise_double_ended_heap(|| MinMaxBinaryArrayDoubleEndedHeap::with_capacity(0));
}

#[test]
fn jheaps_addressable_behavior_binary_and_dary_degrees() {
    exercise_addressable_heap(|| BinaryArrayAddressableHeap::with_capacity(0));
    exercise_addressable_heap(|| DaryArrayAddressableHeap::with_capacity(3, 0).unwrap());
    exercise_addressable_heap(|| DaryArrayAddressableHeap::with_capacity(4, 0).unwrap());
}

#[test]
fn alternate_ord_keys_preserve_addressable_array_heap_ordering() {
    exercise_reverse_addressable_heap(BinaryArrayAddressableHeap::new);
    exercise_reverse_addressable_heap(|| DaryArrayAddressableHeap::new(3).unwrap());
    exercise_reverse_addressable_heap(|| DaryArrayAddressableHeap::new(4).unwrap());

    let entries = (0..STRESS_SIZE)
        .map(|key| (ReverseKey(key), key as usize))
        .collect::<Vec<_>>();
    let mut binary = BinaryArrayAddressableHeap::from_vec(entries.clone());
    assert_eq!(
        binary.pop(),
        Some((ReverseKey(STRESS_SIZE - 1), (STRESS_SIZE - 1) as usize))
    );
    let mut dary = DaryArrayAddressableHeap::from_vec(4, entries).unwrap();
    assert_eq!(
        dary.pop(),
        Some((ReverseKey(STRESS_SIZE - 1), (STRESS_SIZE - 1) as usize))
    );
}

#[test]
fn jheaps_integer_value_heap_behavior_and_properties() {
    exercise_integer_value_heap();
}

#[test]
fn jheaps_heapify_equivalence_for_all_array_heaps() {
    let mut random = JavaRandom::new(1);
    let values = (0..STRESS_SIZE)
        .map(|_| random.next_i32())
        .collect::<Vec<_>>();
    let mut expected = values.clone();
    expected.sort_unstable();

    assert_eq!(drain(BinaryArrayHeap::from_vec(values.clone())), expected);
    assert_eq!(
        drain(DaryArrayHeap::from_vec(2, values.clone()).unwrap()),
        expected
    );
    assert_eq!(
        drain(DaryArrayHeap::from_vec(3, values.clone()).unwrap()),
        expected
    );
    assert_eq!(
        drain(DaryArrayHeap::from_vec(4, values.clone()).unwrap()),
        expected
    );
    assert_eq!(
        drain(DaryArrayHeap::from_vec(5, values.clone()).unwrap()),
        expected
    );
    assert_eq!(
        drain(BinaryArrayWeakHeap::from_vec(values.clone())),
        expected
    );
    assert_eq!(
        drain(BinaryArrayBulkInsertWeakHeap::from_vec(values.clone())),
        expected
    );
    assert_eq!(
        drain(MinMaxBinaryArrayDoubleEndedHeap::from_vec(values.clone())),
        expected
    );
}

#[test]
fn jheaps_heapify_empty_inputs_can_be_reused() {
    assert_constructed_empty(BinaryArrayHeap::from_vec(Vec::new()));
    assert_constructed_empty(DaryArrayHeap::from_vec(2, Vec::new()).unwrap());
    assert_constructed_empty(DaryArrayHeap::from_vec(3, Vec::new()).unwrap());
    assert_constructed_empty(DaryArrayHeap::from_vec(4, Vec::new()).unwrap());
    assert_constructed_empty(DaryArrayHeap::from_vec(5, Vec::new()).unwrap());
    assert_constructed_empty(BinaryArrayWeakHeap::from_vec(Vec::new()));
    assert_constructed_empty(BinaryArrayBulkInsertWeakHeap::from_vec(Vec::new()));
    assert_constructed_empty(MinMaxBinaryArrayDoubleEndedHeap::from_vec(Vec::new()));

    assert_empty_addressable_is_reusable(BinaryArrayAddressableHeap::<i32, i32>::from_vec(
        Vec::new(),
    ));
    assert_empty_addressable_is_reusable(
        DaryArrayAddressableHeap::<i32, i32>::from_vec(3, Vec::new()).unwrap(),
    );
    assert_empty_addressable_is_reusable(
        DaryArrayAddressableHeap::<i32, i32>::from_vec(4, Vec::new()).unwrap(),
    );
    assert_empty_addressable_is_reusable(
        DaryArrayAddressableHeap::<i32, i32>::from_vec(5, Vec::new()).unwrap(),
    );
}

#[test]
fn jheaps_addressable_heapify_preserves_values_and_handles() {
    let mut random = JavaRandom::new(1);
    let entries = (0..STRESS_SIZE)
        .map(|_| {
            let key = random.next_i32();
            (key, key)
        })
        .collect::<Vec<_>>();

    assert_addressable_construction(BinaryArrayAddressableHeap::from_vec(entries.clone()));
    assert_addressable_construction(
        DaryArrayAddressableHeap::from_vec(3, entries.clone()).unwrap(),
    );
    assert_addressable_construction(
        DaryArrayAddressableHeap::from_vec(4, entries.clone()).unwrap(),
    );
    assert_addressable_construction(
        DaryArrayAddressableHeap::from_vec(5, entries.clone()).unwrap(),
    );

    let iterator_entries = (0..STRESS_SIZE)
        .rev()
        .map(|key| (key, key))
        .collect::<Vec<_>>();
    let heap = BinaryArrayAddressableHeap::from_vec(iterator_entries.clone());
    let handles = {
        let mut iterator = heap.handles();
        let handles = iterator.by_ref().collect::<HashSet<_>>();
        assert_eq!(iterator.next(), None);
        handles
    };
    assert_eq!(handles.len(), STRESS_SIZE as usize);
    for handle in handles {
        assert!(heap.key(handle).is_ok());
    }
    assert_eq!(heap.handles().count(), STRESS_SIZE as usize);

    assert_dary_handles_are_live(
        DaryArrayAddressableHeap::from_vec(3, iterator_entries.clone()).unwrap(),
    );
    assert_dary_handles_are_live(DaryArrayAddressableHeap::from_vec(4, iterator_entries).unwrap());
}

#[test]
fn jheaps_addressable_handle_errors_and_reuse() {
    let mut heap = BinaryArrayAddressableHeap::new();
    let first = heap.push(50, 1);
    let second = heap.push(100, 2);
    assert_eq!(heap.pop(), Some((50, 1)));
    assert_eq!(heap.delete(first), Err(InvalidHandle::Stale));
    assert_eq!(
        heap.decrease_key(first, 0),
        Err(DecreaseKeyError::InvalidHandle(InvalidHandle::Stale))
    );
    assert_eq!(heap.delete(second), Ok((100, 2)));
    assert_eq!(heap.delete(second), Err(InvalidHandle::Stale));

    let retained = heap.push(10, 10);
    heap.clear();
    assert_eq!(heap.value(retained), Err(InvalidHandle::Stale));
    let replacement = heap.push(10, 20);
    assert_ne!(retained, replacement);
    assert_eq!(heap.value(replacement), Ok(&20));

    let mut other: BinaryArrayAddressableHeap<i32, i32> = BinaryArrayAddressableHeap::new();
    assert_eq!(other.delete(replacement), Err(InvalidHandle::ForeignHeap));
    assert_eq!(
        other.value_mut(replacement),
        Err(InvalidHandle::ForeignHeap)
    );
    assert_eq!(
        heap.decrease_key(replacement, 11),
        Err(DecreaseKeyError::NotDecreased)
    );
}

#[test]
fn jheaps_degree_validation_is_preserved() {
    assert!(matches!(
        DaryArrayHeap::<i32>::new(0),
        Err(InvalidDegree(0))
    ));
    assert!(matches!(
        DaryArrayHeap::<i32>::new(1),
        Err(InvalidDegree(1))
    ));
    assert!(matches!(
        DaryArrayAddressableHeap::<i32, i32>::new(0),
        Err(InvalidDegree(0))
    ));
    assert!(matches!(
        DaryArrayAddressableHeap::<i32, i32>::new(1),
        Err(InvalidDegree(1))
    ));

    for degree in 2..=5 {
        assert_eq!(
            DaryArrayHeap::from_vec(degree, vec![3, 1, 2])
                .unwrap()
                .degree(),
            degree
        );
        assert_eq!(
            DaryArrayAddressableHeap::from_vec(degree, vec![(3, 3), (1, 1), (2, 2)])
                .unwrap()
                .degree(),
            degree
        );
    }
}
