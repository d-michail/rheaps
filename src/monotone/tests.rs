//! Shared conformance fixtures for every monotone radix heap variant.

use core::fmt::Debug;

use super::{
    BigUint, BigUintRadixAddressableHeap, BigUintRadixHeap, F64RadixAddressableHeap, F64RadixHeap,
    FiniteF64, RadixDecreaseKeyError, RadixHandle, RadixHeapError, U32RadixAddressableHeap,
    U32RadixHeap, U64RadixAddressableHeap, U64RadixHeap,
};
use crate::array::InvalidHandle;
use crate::{AddressableHeap, Heap};

const RANDOM_VALUES: usize = 4_000;

fn finite(value: f64) -> FiniteF64 {
    FiniteF64::new(value).expect("test value is finite")
}

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

    fn next_u64(&mut self) -> u64 {
        self.seed = (self
            .seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::ADDEND))
            & Self::MASK;
        self.seed
    }
}

trait ValueFixture<K> {
    fn checked_push(&mut self, key: K) -> Result<(), RadixHeapError>;
    fn peek_key(&self) -> Option<&K>;
    fn pop_key(&mut self) -> Option<K>;
    fn fixture_len(&self) -> usize;
    fn fixture_clear(&mut self);
}

macro_rules! impl_value_fixture {
    ($heap:ty, $key:ty) => {
        impl ValueFixture<$key> for $heap {
            fn checked_push(&mut self, key: $key) -> Result<(), RadixHeapError> {
                self.push(key)
            }

            fn peek_key(&self) -> Option<&$key> {
                self.peek()
            }

            fn pop_key(&mut self) -> Option<$key> {
                self.pop()
            }

            fn fixture_len(&self) -> usize {
                self.len()
            }

            fn fixture_clear(&mut self) {
                self.clear();
            }
        }
    };
}

impl_value_fixture!(U32RadixHeap, u32);
impl_value_fixture!(U64RadixHeap, u64);
impl_value_fixture!(F64RadixHeap, FiniteF64);
impl_value_fixture!(BigUintRadixHeap, BigUint);

fn exercise_value_heap<H, K>(mut heap: H, values: Vec<K>)
where
    H: ValueFixture<K>,
    K: Clone + Debug + Ord,
{
    assert_eq!(heap.fixture_len(), 0);
    assert_eq!(heap.peek_key(), None);
    assert_eq!(heap.pop_key(), None);

    let mut expected = values.clone();
    expected.sort();
    for value in values {
        heap.checked_push(value).unwrap();
    }
    assert_eq!(heap.fixture_len(), expected.len());
    for expected_key in &expected {
        assert_eq!(heap.peek_key(), Some(expected_key));
        assert_eq!(heap.pop_key().as_ref(), Some(expected_key));
    }
    assert_eq!(heap.pop_key(), None);

    let key = expected
        .first()
        .expect("the fixture must contain at least one key")
        .clone();
    heap.fixture_clear();
    assert_eq!(heap.fixture_len(), 0);
    heap.checked_push(key.clone()).unwrap();
    assert_eq!(heap.pop_key(), Some(key));
}

trait AddressableFixture<K> {
    fn checked_push(&mut self, key: K, value: usize) -> Result<RadixHandle, RadixHeapError>;
    fn fixture_peek(&self) -> Option<(RadixHandle, &K, &usize)>;
    fn fixture_pop(&mut self) -> Option<(K, usize)>;
    fn fixture_key(&self, handle: RadixHandle) -> Result<&K, InvalidHandle>;
    fn fixture_len(&self) -> usize;
    fn fixture_clear(&mut self);
}

macro_rules! impl_addressable_fixture {
    ($heap:ty, $key:ty) => {
        impl AddressableFixture<$key> for $heap {
            fn checked_push(
                &mut self,
                key: $key,
                value: usize,
            ) -> Result<RadixHandle, RadixHeapError> {
                self.push(key, value)
            }

            fn fixture_peek(&self) -> Option<(RadixHandle, &$key, &usize)> {
                self.peek()
            }

            fn fixture_pop(&mut self) -> Option<($key, usize)> {
                self.pop()
            }

            fn fixture_key(&self, handle: RadixHandle) -> Result<&$key, InvalidHandle> {
                self.key(handle)
            }

            fn fixture_len(&self) -> usize {
                self.len()
            }

            fn fixture_clear(&mut self) {
                self.clear();
            }
        }
    };
}

impl_addressable_fixture!(U32RadixAddressableHeap<usize>, u32);
impl_addressable_fixture!(U64RadixAddressableHeap<usize>, u64);
impl_addressable_fixture!(F64RadixAddressableHeap<usize>, FiniteF64);
impl_addressable_fixture!(BigUintRadixAddressableHeap<usize>, BigUint);

fn exercise_addressable_heap<H, K>(mut heap: H, values: Vec<K>)
where
    H: AddressableFixture<K>,
    K: Clone + Debug + Ord,
{
    assert_eq!(heap.fixture_len(), 0);
    assert_eq!(heap.fixture_peek(), None);
    assert_eq!(heap.fixture_pop(), None);

    let mut expected = values.clone();
    expected.sort();
    let mut first_handle = None;
    for (value, key) in values.into_iter().enumerate() {
        let handle = heap.checked_push(key, value).unwrap();
        if first_handle.is_none() {
            first_handle = Some(handle);
        }
    }
    assert_eq!(heap.fixture_len(), expected.len());
    for expected_key in &expected {
        assert_eq!(
            heap.fixture_peek().map(|(_, key, _)| key),
            Some(expected_key)
        );
        assert_eq!(
            heap.fixture_pop().map(|(key, _)| key).as_ref(),
            Some(expected_key)
        );
    }
    assert_eq!(
        heap.fixture_key(first_handle.expect("fixture inserted an initial key")),
        Err(InvalidHandle::Stale)
    );

    let key = expected
        .first()
        .expect("the fixture must contain at least one key")
        .clone();
    heap.fixture_clear();
    let handle = heap.checked_push(key, 0).unwrap();
    heap.fixture_clear();
    assert_eq!(heap.fixture_key(handle), Err(InvalidHandle::Stale));
}

#[test]
fn value_radix_heaps_sort_random_and_boundary_keys() {
    let mut random = JavaRandom::new(1);
    let integers = core::iter::once(0)
        .chain(core::iter::once(u32::MAX))
        .chain((0..RANDOM_VALUES).map(|_| (random.next_u64() % 100_001) as u32))
        .collect();
    exercise_value_heap(U32RadixHeap::new(0, u32::MAX).unwrap(), integers);

    let mut random = JavaRandom::new(2);
    let longs = core::iter::once(0)
        .chain(core::iter::once(u64::MAX))
        .chain((0..RANDOM_VALUES).map(|_| random.next_u64() % 100_001))
        .collect();
    exercise_value_heap(U64RadixHeap::new(0, u64::MAX).unwrap(), longs);

    let mut random = JavaRandom::new(3);
    let doubles = core::iter::once(0.0)
        .chain(core::iter::once(f64::MAX))
        .chain(core::iter::once(f64::MIN_POSITIVE))
        .chain((0..RANDOM_VALUES).map(|_| (random.next_u64() % 100_001) as f64 / 10.0))
        .map(finite)
        .collect();
    exercise_value_heap(
        F64RadixHeap::new(finite(0.0), finite(f64::MAX)).unwrap(),
        doubles,
    );

    let minimum = (BigUint::from(1_u8) << 130_usize) + BigUint::from(7_u8);
    let maximum = minimum.clone() + (BigUint::from(1_u8) << 130_usize);
    let mut random = JavaRandom::new(4);
    let big_integers = core::iter::once(minimum.clone())
        .chain(core::iter::once(maximum.clone()))
        .chain(
            (0..RANDOM_VALUES)
                .map(|_| minimum.clone() + BigUint::from(random.next_u64() % 100_001)),
        )
        .collect();
    exercise_value_heap(
        BigUintRadixHeap::new(minimum, maximum).unwrap(),
        big_integers,
    );
}

#[test]
fn addressable_radix_heaps_sort_random_and_invalidate_handles() {
    let mut random = JavaRandom::new(5);
    let integers = core::iter::once(0)
        .chain((0..RANDOM_VALUES).map(|_| (random.next_u64() % 100_001) as u32))
        .collect();
    exercise_addressable_heap(U32RadixAddressableHeap::new(0, 100_000).unwrap(), integers);

    let mut random = JavaRandom::new(6);
    let longs = core::iter::once(0)
        .chain((0..RANDOM_VALUES).map(|_| random.next_u64() % 100_001))
        .collect();
    exercise_addressable_heap(U64RadixAddressableHeap::new(0, 100_000).unwrap(), longs);

    let mut random = JavaRandom::new(7);
    let doubles = core::iter::once(0.0)
        .chain((0..RANDOM_VALUES).map(|_| (random.next_u64() % 100_001) as f64 / 10.0))
        .map(finite)
        .collect();
    exercise_addressable_heap(
        F64RadixAddressableHeap::new(finite(0.0), finite(10_000.0)).unwrap(),
        doubles,
    );

    let minimum = BigUint::from(1_u8) << 130_usize;
    let maximum = minimum.clone() + BigUint::from(100_000_u32);
    let mut random = JavaRandom::new(8);
    let big_integers = core::iter::once(minimum.clone())
        .chain(
            (0..RANDOM_VALUES)
                .map(|_| minimum.clone() + BigUint::from(random.next_u64() % 100_001)),
        )
        .collect();
    exercise_addressable_heap(
        BigUintRadixAddressableHeap::new(minimum, maximum).unwrap(),
        big_integers,
    );
}

#[test]
fn value_radix_heaps_cover_jheaps_regression_sequences() {
    exercise_value_heap(
        U32RadixHeap::new(29, 36).unwrap(),
        vec![29, 30, 31, 30, 33, 36, 35],
    );
    exercise_value_heap(
        U64RadixHeap::new(29, 36).unwrap(),
        vec![29, 30, 31, 30, 33, 36, 35],
    );
    exercise_value_heap(
        BigUintRadixHeap::new(BigUint::from(29_u8), BigUint::from(36_u8)).unwrap(),
        [29_u8, 30, 31, 30, 33, 36, 35]
            .into_iter()
            .map(BigUint::from)
            .collect(),
    );
    exercise_value_heap(
        F64RadixHeap::new(finite(15.0), finite(50.5)).unwrap(),
        [15.3, 50.4, 20.999_999, 50.5, 30.3, 25.2, 17.777_7]
            .into_iter()
            .map(finite)
            .collect(),
    );

    exercise_value_heap(U32RadixHeap::new(0, u32::MAX).unwrap(), vec![0, u32::MAX]);
    exercise_value_heap(U64RadixHeap::new(0, u64::MAX).unwrap(), vec![0, u64::MAX]);
    exercise_value_heap(
        F64RadixHeap::new(finite(0.0), finite(f64::MAX)).unwrap(),
        vec![finite(0.0), finite(f64::MAX)],
    );

    let mut same = U32RadixHeap::new(15, 15).unwrap();
    for _ in 0..15 {
        same.push(15).unwrap();
    }
    assert_eq!(
        (0..15).map(|_| same.pop()).collect::<Vec<_>>(),
        vec![Some(15); 15]
    );

    let mut after_min = U64RadixHeap::new(0, 15).unwrap();
    after_min.push(0).unwrap();
    assert_eq!(after_min.pop(), Some(0));
    after_min.push(15).unwrap();
    assert_eq!(after_min.peek(), Some(&15));
}

#[test]
fn addressable_radix_heaps_cover_jheaps_regression_sequences() {
    exercise_addressable_heap(
        U32RadixAddressableHeap::new(15, 100).unwrap(),
        vec![15, 50, 21, 51, 30, 25, 18],
    );
    exercise_addressable_heap(
        U64RadixAddressableHeap::new(15, 100).unwrap(),
        vec![15, 50, 21, 51, 30, 25, 18],
    );
    exercise_addressable_heap(
        BigUintRadixAddressableHeap::new(BigUint::from(29_u8), BigUint::from(36_u8)).unwrap(),
        [29_u8, 30, 31, 30, 33, 36, 35]
            .into_iter()
            .map(BigUint::from)
            .collect(),
    );
    exercise_addressable_heap(
        F64RadixAddressableHeap::new(finite(15.0), finite(50.5)).unwrap(),
        [15.3, 50.4, 20.999_999, 50.5, 30.3, 25.2, 17.777_7]
            .into_iter()
            .map(finite)
            .collect(),
    );

    let mut update = U64RadixAddressableHeap::new(0, u64::MAX).unwrap();
    for key in [0, 0, u64::MAX, u64::MAX, u64::MAX, u64::MAX] {
        update.push(key, ()).unwrap();
    }
    assert_eq!(update.pop().map(|entry| entry.0), Some(0));
    assert_eq!(update.pop().map(|entry| entry.0), Some(0));
    assert_eq!(update.peek().map(|entry| entry.1), Some(&u64::MAX));

    let mut regression =
        F64RadixAddressableHeap::new(finite(0.0), finite(3.667_944_409_236_726)).unwrap();
    regression.push(finite(0.0), 0).unwrap();
    regression.push(finite(0.916_986_102_309_181_5), 1).unwrap();
    assert_eq!(regression.pop().map(|entry| entry.0), Some(finite(0.0)));
    regression.push(finite(1.781_470_858_172_715_4), 2).unwrap();
    assert_eq!(
        regression.pop().map(|entry| entry.0),
        Some(finite(0.916_986_102_309_181_5))
    );
    assert_eq!(
        regression.pop().map(|entry| entry.0),
        Some(finite(1.781_470_858_172_715_4))
    );
}

macro_rules! exercise_addressable_handles {
    ($make:expr, $zero:expr, $five:expr, $ten:expr, $fifteen:expr, $twenty:expr, $thirty:expr) => {{
        let mut heap = $make;
        let mut foreign_heap = $make;
        let foreign = foreign_heap.push($zero, 99).unwrap();
        let first = heap.push($zero, 0).unwrap();
        let second = heap.push($ten, 1).unwrap();

        assert_eq!(
            heap.decrease_key(second, $fifteen),
            Err(RadixDecreaseKeyError::NotDecreased)
        );
        heap.decrease_key(second, $five).unwrap();
        *heap.value_mut(first).unwrap() = 42;
        assert_eq!(heap.value(first), Ok(&42));
        assert_eq!(heap.key(foreign), Err(InvalidHandle::ForeignHeap));
        assert_eq!(heap.value_mut(foreign), Err(InvalidHandle::ForeignHeap));

        assert_eq!(heap.pop(), Some(($zero, 42)));
        assert_eq!(heap.key(first), Err(InvalidHandle::Stale));
        assert_eq!(heap.value_mut(first), Err(InvalidHandle::Stale));
        assert_eq!(heap.delete(second), Ok(($five, 1)));
        assert_eq!(heap.key(second), Err(InvalidHandle::Stale));

        heap.push($twenty, 2).unwrap();
        assert_eq!(heap.pop(), Some(($twenty, 2)));
        let live = heap.push($thirty, 3).unwrap();
        assert_eq!(
            heap.decrease_key(live, $fifteen),
            Err(RadixDecreaseKeyError::Radix(
                RadixHeapError::MonotonicityViolation
            ))
        );
        heap.clear();
        assert_eq!(heap.key(live), Err(InvalidHandle::Stale));
        assert_eq!(heap.value_mut(live), Err(InvalidHandle::Stale));
    }};
}

#[test]
fn addressable_heaps_validate_handles_and_monotone_key_updates() {
    exercise_addressable_handles!(
        U32RadixAddressableHeap::new(0, 100).unwrap(),
        0_u32,
        5_u32,
        10_u32,
        15_u32,
        20_u32,
        30_u32
    );
    exercise_addressable_handles!(
        U64RadixAddressableHeap::new(0, 100).unwrap(),
        0_u64,
        5_u64,
        10_u64,
        15_u64,
        20_u64,
        30_u64
    );
    exercise_addressable_handles!(
        F64RadixAddressableHeap::new(finite(0.0), finite(100.0)).unwrap(),
        finite(0.0),
        finite(5.0),
        finite(10.0),
        finite(15.0),
        finite(20.0),
        finite(30.0)
    );
    exercise_addressable_handles!(
        BigUintRadixAddressableHeap::new(BigUint::from(0_u8), BigUint::from(100_u8)).unwrap(),
        BigUint::from(0_u8),
        BigUint::from(5_u8),
        BigUint::from(10_u8),
        BigUint::from(15_u8),
        BigUint::from(20_u8),
        BigUint::from(30_u8)
    );
}

#[test]
fn heaps_report_range_and_monotonicity_errors() {
    assert!(matches!(
        U32RadixHeap::new(2, 1),
        Err(RadixHeapError::InvalidRange)
    ));
    let mut integer = U32RadixHeap::new(10, 20).unwrap();
    assert_eq!(integer.push(9), Err(RadixHeapError::KeyOutOfRange));
    integer.push(15).unwrap();
    assert_eq!(integer.pop(), Some(15));
    assert_eq!(integer.push(14), Err(RadixHeapError::MonotonicityViolation));
    assert_eq!(integer.push(21), Err(RadixHeapError::KeyOutOfRange));

    assert!(matches!(
        U64RadixHeap::new(2, 1),
        Err(RadixHeapError::InvalidRange)
    ));
    let mut long = U64RadixHeap::new(0, 20).unwrap();
    long.push(15).unwrap();
    assert_eq!(long.pop(), Some(15));
    assert_eq!(long.push(14), Err(RadixHeapError::MonotonicityViolation));

    let minimum = BigUint::from(10_u8);
    let mut big = BigUintRadixHeap::new(minimum.clone(), BigUint::from(20_u8)).unwrap();
    big.push(BigUint::from(15_u8)).unwrap();
    assert_eq!(big.pop(), Some(BigUint::from(15_u8)));
    assert_eq!(
        big.push(BigUint::from(14_u8)),
        Err(RadixHeapError::MonotonicityViolation)
    );
    assert!(matches!(
        BigUintRadixHeap::new(BigUint::from(2_u8), BigUint::from(1_u8)),
        Err(RadixHeapError::InvalidRange)
    ));
}

#[test]
fn double_heaps_use_total_order_and_reject_non_finite_keys() {
    assert!(FiniteF64::new(f64::NAN).is_err());
    assert!(FiniteF64::new(f64::INFINITY).is_err());
    assert!(FiniteF64::try_from(f64::NEG_INFINITY).is_err());
    assert!(matches!(
        F64RadixHeap::new(finite(-1.0), finite(1.0)),
        Err(RadixHeapError::InvalidRange)
    ));

    let mut heap = F64RadixHeap::new(finite(-0.0), finite(0.0)).unwrap();
    heap.push(finite(0.0)).unwrap();
    heap.push(finite(-0.0)).unwrap();
    assert_eq!(heap.pop().unwrap().as_f64().to_bits(), (-0.0_f64).to_bits());
    assert_eq!(heap.pop().unwrap().as_f64().to_bits(), 0.0_f64.to_bits());

    let mut values = F64RadixHeap::new(finite(0.0), finite(10.0)).unwrap();
    values.push(finite(0.0)).unwrap();
    assert_eq!(values.pop(), Some(finite(0.0)));
    assert_eq!(
        values.push(finite(-0.0)),
        Err(RadixHeapError::KeyOutOfRange)
    );

    let mut regression = F64RadixHeap::new(finite(0.0), finite(3.667_944_409_236_726)).unwrap();
    regression.push(finite(0.0)).unwrap();
    regression.push(finite(0.916_986_102_309_181_5)).unwrap();
    assert_eq!(regression.pop(), Some(finite(0.0)));
    regression.push(finite(1.781_470_858_172_715_4)).unwrap();
    assert_eq!(regression.pop(), Some(finite(0.916_986_102_309_181_5)));
    assert_eq!(regression.pop(), Some(finite(1.781_470_858_172_715_4)));
}

#[test]
fn common_heap_traits_remain_usable_for_valid_monotone_keys() {
    let mut values = U32RadixHeap::new(0, 10).unwrap();
    Heap::push(&mut values, 3);
    assert_eq!(Heap::pop(&mut values), Some(3));

    let mut entries = U32RadixAddressableHeap::new(0, 10).unwrap();
    let handle = AddressableHeap::push(&mut entries, 3, "entry");
    assert_eq!(AddressableHeap::key(&entries, handle), Ok(&3));
    assert_eq!(AddressableHeap::pop(&mut entries), Some((3, "entry")));
}
