//! Hollow-heap conformance tests adapted from JHeaps' addressable and
//! mergeable addressable heap test bases.

use crate::array::{DecreaseKeyError, InvalidHandle};
use crate::test_support::ReverseKey;
use crate::{AddressableHeap, Heap, MeldableAddressableHeap, MeldableHeap};

use super::{HollowHeap, MeldError};

const STRESS_SIZE: usize = 2_000;

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

fn assert_addressable_trait<H>(heap: &mut H)
where
    H: AddressableHeap<i32, usize>,
{
    let handle = heap.push(2, 2);
    assert_eq!(heap.peek().map(|(_, key, _)| *key), Some(2));
    assert_eq!(heap.delete(handle), Ok((2, 2)));
}

fn assert_meldable_traits<H>(first: &mut H, second: &mut H)
where
    H: MeldableAddressableHeap<i32, usize, MeldError = MeldError>,
{
    first.meld(second).unwrap();
}

#[test]
fn hollow_heap_orders_keys() {
    let mut heap = HollowHeap::<i32, usize>::new();
    assert_addressable_trait(&mut heap);
    assert!(heap.peek_entry().is_none());
    assert!(heap.pop_entry().is_none());

    for value in (0..STRESS_SIZE).rev() {
        heap.insert(value as i32, value);
        assert_eq!(
            heap.peek_entry().map(|(_, key, _)| *key),
            Some(value as i32)
        );
    }
    for expected in 0..STRESS_SIZE {
        assert_eq!(heap.pop_entry(), Some((expected as i32, expected)));
    }
    assert!(heap.is_empty());

    let mut keys_only = HollowHeap::<i32>::new();
    Heap::push(&mut keys_only, 3);
    Heap::push(&mut keys_only, 1);
    Heap::push(&mut keys_only, 2);
    assert_eq!(Heap::peek(&keys_only), Some(&1));
    assert_eq!(Heap::pop(&mut keys_only), Some(1));
    assert_eq!(Heap::pop(&mut keys_only), Some(2));
    assert_eq!(Heap::pop(&mut keys_only), Some(3));

    let mut alternate = HollowHeap::<ReverseKey, usize>::new();
    for value in 0..STRESS_SIZE {
        alternate.insert(ReverseKey(value as i32), value);
    }
    for expected in (0..STRESS_SIZE).rev() {
        assert_eq!(
            alternate.pop_entry(),
            Some((ReverseKey(expected as i32), expected))
        );
    }
}

#[test]
fn hollow_heap_handles_values_decreases_deletions_and_reuse() {
    let mut heap = HollowHeap::<i32, String>::new();
    let handles = (0..128)
        .map(|key| heap.insert(key, key.to_string()))
        .collect::<Vec<_>>();
    heap.assert_invariants();

    *heap.value_mut(handles[7]).unwrap() = "seven".to_owned();
    assert_eq!(heap.value(handles[7]), Ok(&"seven".to_owned()));
    heap.decrease_key(handles[127], -1).unwrap();
    heap.decrease_key(handles[126], -2).unwrap();
    assert_eq!(heap.peek_entry().map(|(_, key, _)| *key), Some(-2));
    assert_eq!(
        heap.decrease_key(handles[7], 1_000),
        Err(DecreaseKeyError::NotDecreased)
    );
    assert_eq!(heap.delete(handles[127]), Ok((-1, "127".to_owned())));
    assert_eq!(heap.key(handles[127]), Err(InvalidHandle::Stale));
    assert_eq!(heap.value_mut(handles[127]), Err(InvalidHandle::Stale));
    heap.assert_invariants();

    assert_eq!(heap.pop_entry(), Some((-2, "126".to_owned())));
    assert_eq!(heap.key(handles[126]), Err(InvalidHandle::Stale));
    heap.assert_invariants();

    let stale = handles[4];
    heap.clear();
    assert!(heap.is_empty());
    assert_eq!(heap.len(), 0);
    assert_eq!(heap.key(stale), Err(InvalidHandle::Stale));
    let reusable = heap.insert(3, "three".to_owned());
    assert_eq!(heap.delete(reusable), Ok((3, "three".to_owned())));
    assert_eq!(heap.key(reusable), Err(InvalidHandle::Stale));

    let mut foreign = HollowHeap::<i32, String>::new();
    let foreign_handle = foreign.insert(5, "five".to_owned());
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

#[test]
fn hollow_heap_survives_random_addressable_operations() {
    let mut heap = HollowHeap::<i32, usize>::new();
    let mut random = Random(3);
    let mut entries = Vec::<Option<(i32, _)>>::new();

    for value in 0..STRESS_SIZE {
        let key = random.next_i32();
        entries.push(Some((key, heap.insert(key, value))));
    }
    heap.assert_invariants();

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
                    assert_eq!(heap.key(handle), Err(InvalidHandle::Stale));
                    entries[index] = None;
                }
                _ => {
                    let expected = entries
                        .iter()
                        .filter_map(|entry| entry.map(|(key, _)| key))
                        .min()
                        .unwrap();
                    let (key, value) = heap.pop_entry().unwrap();
                    assert_eq!(key, expected);
                    assert_eq!(entries[value].map(|(key, _)| key), Some(key));
                    entries[value] = None;
                }
            }
        }
    }

    let mut previous = None;
    while let Some((key, value)) = heap.pop_entry() {
        assert_eq!(entries[value].map(|(expected, _)| expected), Some(key));
        entries[value] = None;
        if let Some(previous) = previous {
            assert!(previous <= key);
        }
        previous = Some(key);
    }
    assert!(entries.iter().all(Option::is_none));
    heap.assert_invariants();
}

#[test]
fn hollow_heap_reclaims_hollow_nodes_after_many_decreases_and_deletes() {
    let mut heap = HollowHeap::<i32, usize>::new();
    let handles = (0..STRESS_SIZE)
        .map(|index| heap.insert((index * 2) as i32, index))
        .collect::<Vec<_>>();

    for index in (0..STRESS_SIZE).rev() {
        heap.decrease_key(handles[index], index as i32).unwrap();
    }
    heap.assert_invariants();

    for index in (1..STRESS_SIZE).step_by(7) {
        assert_eq!(heap.delete(handles[index]), Ok((index as i32, index)));
    }
    heap.assert_invariants();

    let mut previous = None;
    while let Some((key, _)) = heap.pop_entry() {
        if let Some(previous) = previous {
            assert!(previous <= key);
        }
        previous = Some(key);
    }
    assert!(heap.is_empty());
    heap.assert_invariants();
}

#[test]
fn hollow_heap_handles_bulk_deletion_and_reverse_decreases() {
    let mut heap = HollowHeap::<i32, usize>::new();
    let handles = (0..STRESS_SIZE)
        .map(|index| heap.insert(index as i32, index))
        .collect::<Vec<_>>();
    for index in (0..STRESS_SIZE).rev() {
        assert_eq!(heap.delete(handles[index]), Ok((index as i32, index)));
        if index > 0 {
            assert_eq!(heap.peek_entry().map(|(_, key, _)| *key), Some(0));
        }
    }
    assert!(heap.is_empty());
    heap.assert_invariants();

    let mut alternate = HollowHeap::<ReverseKey, usize>::new();
    let handles = (0..STRESS_SIZE)
        .map(|index| alternate.insert(ReverseKey(index as i32), index))
        .collect::<Vec<_>>();
    for (index, handle) in handles.iter().copied().enumerate() {
        alternate
            .decrease_key(handle, ReverseKey((index + STRESS_SIZE) as i32))
            .unwrap();
    }
    for expected in (STRESS_SIZE..STRESS_SIZE * 2).rev() {
        assert_eq!(
            alternate.pop_entry(),
            Some((ReverseKey(expected as i32), expected - STRESS_SIZE))
        );
    }
    alternate.assert_invariants();
}

#[test]
fn hollow_heap_melds_move_handle_domains_and_consume_donors() {
    let mut a = HollowHeap::<i32, usize>::new();
    let mut b = HollowHeap::<i32, usize>::new();
    let mut c = HollowHeap::<i32, usize>::new();
    let b_handle = b.insert(17, 17);
    let c_handle = c.insert(28, 28);
    for value in 0..100 {
        a.insert((value * 3) as i32, value);
        b.insert((value * 3 + 1) as i32, value + 100);
        c.insert((value * 3 + 2) as i32, value + 200);
    }

    assert_meldable_traits(&mut a, &mut b);
    assert_eq!(b.try_insert(3, 3), Err(MeldError::ReceiverConsumed));
    assert_eq!(b.key(b_handle), Err(InvalidHandle::ForeignHeap));
    assert_eq!(a.key(b_handle), Ok(&17));
    assert_eq!(a.meld(&mut b), Err(MeldError::DonorConsumed));

    a.meld(&mut c).unwrap();
    assert_eq!(c.key(c_handle), Err(InvalidHandle::ForeignHeap));
    a.decrease_key(b_handle, -1).unwrap();
    a.decrease_key(c_handle, -2).unwrap();
    assert_eq!(a.peek_entry().map(|(_, key, _)| *key), Some(-2));
    assert_eq!(a.delete(c_handle), Ok((-2, 28)));
    assert_eq!(a.key(c_handle), Err(InvalidHandle::Stale));
    a.assert_invariants();

    let mut previous = None;
    while let Some((key, _)) = a.pop_entry() {
        if let Some(previous) = previous {
            assert!(previous <= key);
        }
        previous = Some(key);
    }
    a.assert_invariants();

    let mut keys_a = HollowHeap::<i32>::new();
    let mut keys_b = HollowHeap::<i32>::new();
    Heap::push(&mut keys_a, 2);
    Heap::push(&mut keys_b, 1);
    MeldableHeap::meld(&mut keys_a, &mut keys_b).unwrap();
    assert_eq!(Heap::pop(&mut keys_a), Some(1));
    assert_eq!(Heap::pop(&mut keys_a), Some(2));

    let mut chain_a = HollowHeap::<i32, usize>::new();
    let mut chain_b = HollowHeap::<i32, usize>::new();
    let mut chain_c = HollowHeap::<i32, usize>::new();
    let mut chain_d = HollowHeap::<i32, usize>::new();
    let carried = chain_d.insert(29, 29);
    chain_c.meld(&mut chain_d).unwrap();
    chain_b.meld(&mut chain_c).unwrap();
    chain_a.meld(&mut chain_b).unwrap();
    assert_eq!(chain_a.key(carried), Ok(&29));
    chain_a.decrease_key(carried, -3).unwrap();
    assert_eq!(chain_a.pop_entry(), Some((-3, 29)));
    assert_eq!(chain_a.key(carried), Err(InvalidHandle::Stale));
}
