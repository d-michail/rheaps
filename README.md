# rheaps

`rheaps` is an incremental, idiomatic Rust port of
[JHeaps](https://github.com/d-michail/jheaps), an Apache-2.0-licensed
collection of priority queues.

## Available now

- Complete array-backed heap family: binary and configurable d-ary min-heaps,
  binary weak heaps (including bulk insertion), addressable binary and d-ary
  heaps, integer-key value heaps, and min-max double-ended heaps
- Complete monotone radix heap family: `u32`, `u64`, finite `FiniteF64`, and
  arbitrary-sized unsigned (`num_bigint::BigUint`) key heaps, each with an
  addressable counterpart and checked monotonic-key operations
- Complete tree heap family: explicit binary and power-of-two d-ary
  addressable heaps; Kaplan-Zwick binary-tree soft heaps; leftist, skew,
  pairing, pure/rank/costless-meld pairing, and Fibonacci/simple/strict
  Fibonacci heaps; plus reflected Fibonacci and pairing double-ended heaps
- Complete DAG heap family: addressable, meldable hollow heaps with lazy
  hollow-node reclamation
- Heaps use the key type's `Ord` implementation, with checked opaque handles
  for addressable heaps. Reflected heaps implement min/max addressable
  operations, including `increase_key`, through `DoubleEndedAddressableHeap`.
  To use another priority order, wrap the key in a newtype and implement `Ord`
  with that order (for example, reverse it for max-oriented behavior).

All public heap implementations from the Java JHeaps source are now ported.

`DoubleRadixHeap` and `DoubleRadixAddressableHeap` take `FiniteF64` keys.
Construct them with `FiniteF64::new(value)` or `value.try_into()`; invalid
NaN and infinite values are rejected before insertion.

## Porting order

1. ~~Array heaps: binary, d-ary, weak, min-max, and addressable variants~~
2. ~~Monotone radix heaps~~
3. ~~Tree heaps: explicit binary/d-ary addressable, soft, leftist, skew,
   pairing, Fibonacci, simple Fibonacci, strict Fibonacci,
   pure/rank/costless-meld pairing, and reflected variants~~
4. ~~DAG heap: hollow heap~~

The crate uses Rust ownership rather than Java exceptions: `peek` returns
`Option` and `pop` returns owned values. Addressable heaps use checked opaque
handles; removed entries and cleared heaps invalidate their handles.
