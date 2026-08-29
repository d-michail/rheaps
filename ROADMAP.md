# Roadmap

This roadmap tracks opportunities to make `rheaps` feel more native to Rust
after its initial port from [JHeaps](https://github.com/d-michail/jheaps).
The algorithms and their behavioral guarantees should remain intact; the work
below concerns API design, ownership, naming, error modeling, and integration
with the Rust ecosystem.

None of the items below is a commitment to break compatibility. Before the
crate reaches a stable release, changes can be made directly. After that,
renames and signature changes should follow a deprecation and migration cycle.

## Guiding principles

- Express invalid states and one-way transitions through ownership where
  practical.
- Keep fallible operations visibly fallible.
- Keep shared abstractions independent of individual implementation modules.
- Prefer one canonical name for each operation.
- Use names that identify Rust types rather than Java primitive types.
- Integrate naturally with standard iterator and collection traits.
- Preserve the full JHeaps-derived behavioral test coverage during each
  migration.

## Priority 1: Ownership and error semantics

### Consume meld donors through ownership

Current meld operations take `&mut Self`, transfer the donor's contents, and
leave the donor in an inactive state. Subsequent operations can therefore fail
with `ReceiverConsumed` or `DonorConsumed`.

Investigate changing the common operation to consume the donor:

```rust
fn meld(&mut self, other: Self) -> Result<(), Self::MeldError>;
```

This would make donor reuse a compile-time error and could remove the inactive
heap state and its associated runtime errors. Addressable donor handles must
remain valid through the receiver after the move.

Tasks:

- Update `MeldableHeap`, `MeldableAddressableHeap`, and
  `MeldableDoubleEndedAddressableHeap`.
- Update tree, reflected, soft, and hollow heap implementations.
- Remove `active` state where it is no longer required.
- Reassess `ReceiverConsumed` and `DonorConsumed` error variants.
- Preserve tests for transitive melds and donor-handle migration.

### Introduce a fallible heap abstraction

Radix heaps have fallible insertion because keys can violate configured bounds
or monotonicity. Their `Heap` and `AddressableHeap` implementations currently
adapt this to an infallible `push` by panicking.

Investigate a `TryHeap` abstraction, associated insertion errors on the common
traits, or deliberately omitting infallible trait implementations for radix
heaps. Valid generic operations should not unexpectedly panic because a heap
has algorithm-specific key restrictions.

Tasks:

- Design fallible equivalents for value-less and addressable insertion.
- Decide whether infallible common-trait implementations remain useful.
- Ensure radix range and monotonicity errors retain their detail.
- Add compile-time and runtime examples for generic fallible use.

### Decouple common traits from array errors

The crate-root addressable traits currently return error types declared in the
`array` module, even when implemented by tree, DAG, and monotone heaps.

Tasks:

- Move universally shared handle errors to the crate root or a neutral
  `error` module, or introduce associated error types.
- Determine whether `DecreaseKeyError::Unsupported` indicates that key
  decrease belongs in a separate capability trait.
- Keep conversions between specialized radix errors and common errors only
  where they do not discard useful information.
- Update documentation so errors describe capabilities rather than their
  original implementation family.

## Priority 2: Public API consolidation

### Choose canonical insertion terminology

Many heaps expose both `push` and `insert`; radix heaps expose `push`,
`try_push`, `insert`, and `try_insert`. Several aliases exist only to match
JHeaps terminology.

Tasks:

- Select one canonical insertion name for each common abstraction.
- Reserve a `try_*` name for cases with a real fallible/infallible distinction.
- Deprecate redundant aliases if compatibility must be maintained.
- Use the canonical vocabulary consistently in examples and documentation.

### Consolidate value-less and entry APIs

Tree heaps use `V = ()` for value-less heaps while also exposing parallel
method families such as `push`/`peek`/`pop` and
`insert`/`peek_entry`/`pop_entry`. The crate root similarly separates `Heap`,
`ValueHeap`, and `AddressableHeap` along lines inherited from JHeaps.

Tasks:

- Evaluate whether an entry-oriented abstraction or associated value type can
  reduce duplication.
- Keep value-less heaps ergonomic without requiring callers to mention `()`.
- Avoid ordering `(K, V)` tuples by their values when only keys determine
  priority.
- Preserve ownership-friendly borrowed peeks and owned removals.

### Hide reflected-heap backend details

`InnerRecord`, reflected backend marker types, and `ReflectedHeapBackend` are
public implementation details hidden from generated documentation because the
generic `ReflectedHeap` exposes them in its bounds.

Tasks:

- Consider sealed backends, private implementation modules, or distinct public
  concrete reflected heap types.
- Keep `ReflectedFibonacciHeap` and `ReflectedPairingHeap` easy to name and
  construct.
- Minimize public types that exist only to satisfy visibility rules.

## Priority 3: Rust naming and ecosystem integration

### Rename Java primitive-oriented radix heaps

The current names reflect Java primitive and class names rather than their
Rust key types:

| Current name | Key type | Candidate Rust name |
| --- | --- | --- |
| `IntegerRadixHeap` | `u32` | `U32RadixHeap` |
| `LongRadixHeap` | `u64` | `U64RadixHeap` |
| `DoubleRadixHeap` | `FiniteF64` | `F64RadixHeap` |
| `BigIntegerRadixHeap` | `BigUint` | `BigUintRadixHeap` |

The same change applies to addressable variants.

Tasks:

- Confirm final names against Rust ecosystem conventions.
- Introduce aliases and deprecate old names if compatibility requires it.
- Update examples, documentation, and error messages.
- Avoid implying support for signed integer ranges where keys are unsigned.

### Implement standard collection traits

Array heaps currently use constructors such as `from_vec` and conversions such
as `into_vec`, but have limited integration with Rust's iterator ecosystem.

Tasks:

- Implement `FromIterator` for heaps that can be built from arbitrary input.
- Implement `Extend` with behavior consistent with repeated insertion.
- Evaluate consuming and borrowed `IntoIterator` implementations.
- Document whether iteration follows priority order or internal heap order.
- Retain linear-time heap construction where the algorithm supports it.

## Cross-cutting work

Every API migration should include:

- conformance tests for natural and reversed `Ord` implementations;
- addressable handle validation, invalidation, and migration tests;
- random-operation and duplicate-key coverage;
- rustdoc examples for the preferred API;
- migration notes for renamed or removed methods; and
- formatting, Clippy, unit-test, doc-test, and rustdoc checks in CI.

## Suggested sequence

1. Decide the pre-1.0 compatibility policy.
2. Redesign meld ownership and remove poisoned donor states.
3. Design fallible heap traits and neutral shared error types.
4. Consolidate insertion and entry method families.
5. Seal reflected-heap implementation details.
6. Introduce Rust-oriented radix names.
7. Add standard iterator and collection trait implementations.
8. Publish a migration guide and stabilize the resulting public API.
