# rheaps

[![CI](https://github.com/d-michail/rheaps/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/d-michail/rheaps/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/d-michail/rheaps)](LICENSE)

`rheaps` is a collection of heap and priority-queue data structures written in
Rust. It is an idiomatic Rust port of the
[JHeaps](https://github.com/d-michail/jheaps) library and includes array,
tree, DAG, double-ended, addressable, meldable, soft, and monotone heaps behind
a small set of common traits.

The crate is licensed under the Apache License, Version 2.0.

## What is this library?

This library provides several heap implementations with well-defined Rust
interfaces. It is intended for applications and experiments that need more
than a conventional binary priority queue—for example, stable handles,
efficient melding, access to both extrema, monotone keys, or a particular heap
algorithm.

- Keys use their `Ord` implementation, and duplicate keys are permitted.
- Queries borrow their result; removals return owned values.
- Empty-heap operations return `None`.
- Addressable heaps return checked opaque handles.
- Meldable heaps preserve handles from the donor heap after a successful meld.
- Constructors validate parameters such as d-ary degrees, soft-heap error
  rates, and radix-heap key bounds.

## What is a heap?

A heap is a priority queue containing elements whose keys come from a totally
ordered set. A min-oriented heap supports these core operations:

- create an empty heap;
- insert an element;
- inspect the element with the smallest key;
- remove the element with the smallest key;
- query its size or whether it is empty; and
- remove all elements.

Some implementations provide additional operations:

- **Addressable heaps** return handles that can be used to inspect, update, or
  delete individual entries.
- **Meldable heaps** efficiently combine two heaps of the same concrete type.
- **Double-ended heaps** expose both the minimum and maximum.
- **Monotone heaps** exploit the guarantee that newly inserted keys are not
  smaller than the last key removed.
- **Soft heaps** permit controlled key corruption in exchange for useful
  amortized performance bounds.

## Installation

The crate uses Rust 2024 edition. To use the current repository version, add:

```toml
[dependencies]
rheaps = { git = "https://github.com/d-michail/rheaps" }
```

To build and test a checkout:

```console
git clone https://github.com/d-michail/rheaps.git
cd rheaps
cargo test --all-targets
```

## Quick start

All ordinary heaps are min-oriented according to the key type's `Ord`
implementation.

```rust
use rheaps::Heap;
use rheaps::array::BinaryArrayHeap;

let mut heap = BinaryArrayHeap::new();
heap.push(4);
heap.push(1);
heap.push(3);

assert_eq!(heap.peek(), Some(&1));
assert_eq!(heap.pop(), Some(1));
assert_eq!(heap.pop(), Some(3));
assert_eq!(heap.pop(), Some(4));
assert_eq!(heap.pop(), None);
```

### Addressable heaps

An addressable heap associates each key with a value and returns a handle for
the entry. Handles are rejected if they are stale or belong to another heap.

```rust
use rheaps::AddressableHeap;
use rheaps::array::BinaryArrayAddressableHeap;

let mut heap = BinaryArrayAddressableHeap::new();
let task = heap.push(10, "compile report");
heap.push(5, "answer mail");

heap.decrease_key(task, 1).unwrap();
assert_eq!(
    heap.peek().map(|(_, key, value)| (*key, *value)),
    Some((1, "compile report")),
);
assert_eq!(heap.delete(task), Ok((1, "compile report")));
```

### Alternative ordering

To select a different priority order, wrap the key in a type with the desired
`Ord` implementation. The standard library's `Reverse` wrapper turns any
min-oriented heap into a max-oriented heap.

```rust
use std::cmp::Reverse;
use rheaps::Heap;
use rheaps::array::BinaryArrayHeap;

let mut heap = BinaryArrayHeap::new();
heap.push(Reverse(1));
heap.push(Reverse(4));
heap.push(Reverse(3));

assert_eq!(heap.pop(), Some(Reverse(4)));
```

### Monotone radix heaps

Radix heaps require explicit inclusive key bounds and reject insertions that
fall outside those bounds or violate monotonicity.

```rust
use rheaps::monotone::U32RadixHeap;

let mut heap = U32RadixHeap::new(0, 1_000).unwrap();
heap.push(12).unwrap();
heap.push(7).unwrap();
assert_eq!(heap.pop(), Some(7));

// After removing 7, keys below 7 are no longer valid.
assert!(heap.push(6).is_err());
```

Floating-point radix heaps use `FiniteF64`, which provides a total order and
rejects NaN and infinite values before insertion.

```rust
use rheaps::monotone::{F64RadixHeap, FiniteF64};

let zero = FiniteF64::new(0.0).unwrap();
let ten = FiniteF64::new(10.0).unwrap();
let mut heap = F64RadixHeap::new(zero, ten).unwrap();
heap.push(FiniteF64::new(2.5).unwrap()).unwrap();
assert_eq!(heap.pop().map(FiniteF64::into_inner), Some(2.5));
```

## Implementations

### Array-based

- `BinaryArrayHeap`
- `DaryArrayHeap`
- `BinaryArrayAddressableHeap`
- `DaryArrayAddressableHeap`
- `BinaryArrayWeakHeap`
- `BinaryArrayBulkInsertWeakHeap`
- `BinaryArrayIntegerValueHeap`
- `MinMaxBinaryArrayDoubleEndedHeap`

### Tree-based

- `BinaryTreeAddressableHeap`
- `DaryTreeAddressableHeap`
- `BinaryTreeSoftHeap`
- `BinaryTreeSoftAddressableHeap`
- `LeftistHeap`
- `SkewHeap`
- `PairingHeap`
- `PurePairingHeap`
- `RankPairingHeap`
- `CostlessMeldPairingHeap`
- `FibonacciHeap`
- `SimpleFibonacciHeap`
- `StrictFibonacciHeap`

The leftist, skew, pairing, and Fibonacci families are addressable and
meldable. The soft and explicit tree heaps expose the capabilities appropriate
to their algorithms.

### DAG-based

- `HollowHeap`, an addressable and meldable hollow heap with lazy reclamation
  of hollow nodes

### Double-ended

- `MinMaxBinaryArrayDoubleEndedHeap`
- `ReflectedFibonacciHeap`
- `ReflectedPairingHeap`

The reflected heaps are addressable and meldable and support minimum and
maximum access together with both `decrease_key` and `increase_key`.

### Monotone

Each supported key family has value-less and addressable variants:

- `U32RadixHeap` and `U32RadixAddressableHeap` for `u32`;
- `U64RadixHeap` and `U64RadixAddressableHeap` for `u64`;
- `F64RadixHeap` and `F64RadixAddressableHeap` for `FiniteF64`; and
- `BigUintRadixHeap` and `BigUintRadixAddressableHeap` for
  `num_bigint::BigUint`.

## Common interfaces

The crate root defines the shared traits:

- `Heap` and `ValueHeap`;
- `AddressableHeap`;
- `DoubleEndedHeap` and `DoubleEndedAddressableHeap`;
- `MeldableHeap` and `MeldableAddressableHeap`; and
- `MeldableDoubleEndedAddressableHeap`.

Concrete types also provide inherent methods, so callers can use a heap
directly without writing generic code. Addressable operations report invalid,
foreign, and stale handles explicitly. Removing an entry or clearing a heap
invalidates its handle. A successful meld consumes the donor heap for future
mutation while allowing its existing handles to be used through the receiver.

Array-backed heaps implement `FromIterator` and `Extend`. Collecting into a
d-ary heap uses the binary degree of two; extending an existing d-ary heap
preserves its configured degree. Addressable variants collect and extend
`(key, value)` pairs.

## Relationship to JHeaps

The implementation set and much of the behavioral test coverage are derived
from [JHeaps](https://github.com/d-michail/jheaps). The API follows Rust's
ownership, trait, and error-handling conventions rather than reproducing the
Java API literally. All public heap implementations in JHeaps are represented
in this crate.

## License

Copyright (C) 2014–2026 Dimitrios Michail

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.
