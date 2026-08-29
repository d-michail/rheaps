//! Shared tree-heap conformance fixtures patterned after the array suite.

use core::cmp::Ordering;

use crate::array::{DecreaseKeyError, InvalidHandle};
use crate::{AddressableHeap, Heap};

use super::{BinaryTreeAddressableHeap, LeftistHeap, MeldError, PairingHeap, SkewHeap, TreeHandle};

const STRESS_SIZE: i32 = 2_000;

type Reverse = fn(&i32, &i32) -> Ordering;

fn reverse(left: &i32, right: &i32) -> Ordering {
    right.cmp(left)
}

fn reverse_again(left: &i32, right: &i32) -> Ordering {
    right.cmp(left)
}

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

fn exercise_heap<H>(make: impl Fn() -> H, reverse: bool)
where
    H: Heap<i32>,
{
    let mut heap = make();
    assert_eq!(heap.peek(), None);
    assert_eq!(heap.pop(), None);
    for value in (0..STRESS_SIZE).rev() {
        heap.push(value);
    }
    for offset in 0..STRESS_SIZE {
        let expected = if reverse {
            STRESS_SIZE - offset - 1
        } else {
            offset
        };
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
            assert!(
                if reverse {
                    previous >= value
                } else {
                    previous <= value
                },
                "tree heap output was not ordered"
            );
        }
        previous = Some(value);
    }
}

fn exercise_addressable_heap<H>(make: impl Fn() -> H + Copy, reverse: bool)
where
    H: AddressableHeap<i32, usize, Handle = TreeHandle>,
{
    let mut heap = make();
    assert_eq!(heap.peek(), None);
    assert_eq!(heap.pop(), None);

    let handles = (0..256)
        .map(|key| heap.push(key, key as usize))
        .collect::<Vec<_>>();
    for key in [17, 5, 255, 0, 73, 109, 1, 200] {
        assert_eq!(heap.delete(handles[key]), Ok((key as i32, key)));
        assert_eq!(heap.key(handles[key]), Err(InvalidHandle::Stale));
    }

    let handle = heap.push(1_000, 1_000);
    let decreased = if reverse { 2_000 } else { -1 };
    heap.decrease_key(handle, decreased).unwrap();
    assert_eq!(heap.key(handle), Ok(&decreased));
    let invalid = if reverse { -1_000 } else { 2_000 };
    assert_eq!(
        heap.decrease_key(handle, invalid),
        Err(DecreaseKeyError::NotDecreased)
    );
    heap.set_value(handle, 7).unwrap();
    assert_eq!(heap.value(handle), Ok(&7));

    let mut previous = None;
    while let Some((key, _)) = heap.pop() {
        if let Some(previous) = previous {
            assert!(if reverse {
                previous >= key
            } else {
                previous <= key
            });
        }
        previous = Some(key);
    }
    assert_eq!(heap.key(handle), Err(InvalidHandle::Stale));

    let stale = heap.push(4, 4);
    heap.clear();
    assert_eq!(heap.key(stale), Err(InvalidHandle::Stale));
    assert_eq!(heap.pop(), None);

    let mut foreign = make();
    let foreign_handle = foreign.push(5, 5);
    assert_eq!(heap.key(foreign_handle), Err(InvalidHandle::ForeignHeap));
}

fn exercise_addressable_random_operations<H>(make: impl Fn() -> H, reverse: bool)
where
    H: AddressableHeap<i32, usize, Handle = TreeHandle>,
{
    let mut heap = make();
    let mut random = Random(3);
    let mut entries = Vec::<Option<(i32, TreeHandle)>>::new();

    for value in 0..STRESS_SIZE as usize {
        let key = random.next_i32();
        entries.push(Some((key, heap.push(key, value))));
    }

    for _ in 0..STRESS_SIZE {
        let index = (random.next_i32() as u32 as usize) % entries.len();
        if let Some((key, handle)) = entries[index] {
            match random.next_i32() & 3 {
                0 => {
                    let next = if reverse {
                        key.saturating_add((random.next_i32() as u32 & 255) as i32)
                    } else {
                        key.saturating_sub((random.next_i32() as u32 & 255) as i32)
                    };
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
                        .min_by(|left, right| {
                            if reverse {
                                right.cmp(left)
                            } else {
                                left.cmp(right)
                            }
                        })
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
            assert!(if reverse {
                previous >= key
            } else {
                previous <= key
            });
        }
        previous = Some(key);
    }
    assert!(entries.iter().all(Option::is_none));
}

fn exercise_binary_addressable_heap<H>(make: impl Fn() -> H + Copy, reverse: bool)
where
    H: AddressableHeap<i32, usize, Handle = TreeHandle>,
{
    exercise_addressable_heap(make, reverse);
    let mut random = Random(2);
    let mut heap = make();
    let mut keys = Vec::new();
    let mut handles = Vec::new();
    for value in 0..STRESS_SIZE {
        let key = random.next_i32();
        keys.push(key);
        handles.push(heap.push(key, value as usize));
    }
    for index in (0..STRESS_SIZE as usize).step_by(7) {
        let next = if reverse {
            keys[index].saturating_add(10)
        } else {
            keys[index].saturating_sub(10)
        };
        heap.decrease_key(handles[index], next).unwrap();
    }
    let mut previous = None;
    while let Some((key, _)) = heap.pop() {
        if let Some(previous) = previous {
            assert!(if reverse {
                previous >= key
            } else {
                previous <= key
            });
        }
        previous = Some(key);
    }
}

macro_rules! exercise_meld {
    ($heap:ident) => {{
        let mut first = $heap::<i32, usize>::new();
        let mut second = $heap::<i32, usize>::new();
        for value in 0..100 {
            first.push(value * 2, value as usize);
        }
        let donor_handle = second.push(201, 201);
        for value in 0..100 {
            second.push(value * 2 + 1, value as usize);
        }
        first.meld(&mut second).unwrap();
        assert!(second.is_empty());
        assert_eq!(second.try_insert(3, 3), Err(MeldError::ReceiverConsumed));
        assert_eq!(second.key(donor_handle), Err(InvalidHandle::ForeignHeap));
        assert_eq!(first.meld(&mut second), Err(MeldError::DonorConsumed));
        first.decrease_key(donor_handle, -1).unwrap();
        assert_eq!(first.peek().map(|(_, key, _)| *key), Some(-1));
        let mut expected = -1;
        while let Some((key, _)) = first.pop() {
            assert!(expected <= key);
            expected = key;
        }

        let mut a = $heap::<i32, usize>::new();
        let mut b = $heap::<i32, usize>::new();
        let mut c = $heap::<i32, usize>::new();
        let handle = c.push(10, 10);
        b.meld(&mut c).unwrap();
        a.meld(&mut b).unwrap();
        a.decrease_key(handle, 0).unwrap();
        assert_eq!(a.peek().map(|(_, key, _)| *key), Some(0));
    }};
}

#[test]
fn tree_heaps_follow_common_heap_and_comparator_conformance() {
    exercise_heap(LeftistHeap::<i32>::new, false);
    exercise_heap(SkewHeap::<i32>::new, false);
    exercise_heap(PairingHeap::<i32>::new, false);
    exercise_heap(
        || LeftistHeap::<i32, (), Reverse>::with_comparator(reverse as Reverse),
        true,
    );
    exercise_heap(
        || SkewHeap::<i32, (), Reverse>::with_comparator(reverse as Reverse),
        true,
    );
    exercise_heap(
        || PairingHeap::<i32, (), Reverse>::with_comparator(reverse as Reverse),
        true,
    );
}

#[test]
fn tree_addressable_heaps_enforce_handle_rules() {
    exercise_addressable_heap(LeftistHeap::<i32, usize>::new, false);
    exercise_addressable_heap(SkewHeap::<i32, usize>::new, false);
    exercise_addressable_heap(PairingHeap::<i32, usize>::new, false);
    exercise_binary_addressable_heap(BinaryTreeAddressableHeap::<i32, usize>::new, false);
    exercise_addressable_heap(
        || LeftistHeap::<i32, usize, Reverse>::with_comparator(reverse as Reverse),
        true,
    );
    exercise_addressable_heap(
        || SkewHeap::<i32, usize, Reverse>::with_comparator(reverse as Reverse),
        true,
    );
    exercise_addressable_heap(
        || PairingHeap::<i32, usize, Reverse>::with_comparator(reverse as Reverse),
        true,
    );
    exercise_binary_addressable_heap(
        || BinaryTreeAddressableHeap::<i32, usize, Reverse>::with_comparator(reverse as Reverse),
        true,
    );

    exercise_addressable_random_operations(LeftistHeap::<i32, usize>::new, false);
    exercise_addressable_random_operations(SkewHeap::<i32, usize>::new, false);
    exercise_addressable_random_operations(PairingHeap::<i32, usize>::new, false);
    exercise_addressable_random_operations(BinaryTreeAddressableHeap::<i32, usize>::new, false);
}

#[test]
fn tree_melds_consume_donor_and_preserve_handles() {
    exercise_meld!(LeftistHeap);
    exercise_meld!(SkewHeap);
    exercise_meld!(PairingHeap);
}

#[test]
fn tree_meld_rejects_distinct_comparators() {
    let mut first = PairingHeap::<i32, usize, Reverse>::with_comparator(reverse as Reverse);
    let mut second = PairingHeap::<i32, usize, Reverse>::with_comparator(reverse_again as Reverse);
    first.push(1, 1);
    second.push(2, 2);
    assert_eq!(
        first.meld(&mut second),
        Err(MeldError::IncompatibleComparator)
    );
    assert_eq!(first.pop(), Some((1, 1)));
    assert_eq!(second.pop(), Some((2, 2)));
}
