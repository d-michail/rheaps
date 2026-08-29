# rheaps

`rheaps` is an incremental, idiomatic Rust port of
[JHeaps](https://github.com/d-michail/jheaps), an Apache-2.0-licensed
collection of priority queues.

## Available now

- Complete array-backed heap family: binary and configurable d-ary min-heaps,
  binary weak heaps (including bulk insertion), addressable binary and d-ary
  heaps, integer-key value heaps, and min-max double-ended heaps
- Complete monotone radix heap family: `u32`, `u64`, finite `f64`, and
  arbitrary-sized unsigned (`num_bigint::BigUint`) key heaps, each with an
  addressable counterpart and checked monotonic-key operations
- Tree heaps: leftist, skew, pairing, pure/rank/costless-meld pairing, and
  Fibonacci/simple/strict Fibonacci heaps (all with addressable handles and
  meld support), plus the explicit binary-tree addressable heap
- Natural or custom ordering where supported, with checked opaque handles for
  addressable heaps

## Porting order

1. ~~Array heaps: binary, d-ary, weak, min-max, and addressable variants~~
2. ~~Monotone radix heaps~~
3. Tree heaps: ~~leftist, skew, pairing, Fibonacci, simple Fibonacci,
   strict Fibonacci, pure/rank/costless-meld pairing, and explicit binary-tree
   addressable heaps~~; soft, reflected, and related variants remain
4. DAG and remaining double-ended heaps

The crate uses Rust ownership rather than Java exceptions: `peek` returns
`Option` and `pop` returns owned values. Addressable heaps use checked opaque
handles; removed entries and cleared heaps invalidate their handles.
