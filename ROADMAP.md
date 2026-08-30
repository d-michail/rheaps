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

## Medium-priority JHeaps test-port follow-up

The highest-risk behavioral gaps from the JHeaps suite are covered: d-ary
addressable heaps run at degrees 2, 4, 8, and 16; meldable heaps exercise
empty, even, odd, reusable, and cascading cases; strict Fibonacci heaps run
the 2,000, 4,000, and 20,000-operation randomized workloads; radix heaps have
explicit small-range and historical-regression cases; and soft heaps exercise
all five original error-rate bands, uneven melds, deletion, and cascading
addressable melds.

The remaining medium-priority work is primarily test organization and explicit
traceability rather than missing algorithmic scenarios:

- Maintain a method-level mapping from JHeaps test names to the shared Rust
  conformance fixtures. Rust intentionally tests many heap implementations
  through one fixture instead of duplicating a Java test class per heap.
- Split unusually broad conformance tests when a failure would otherwise be
  difficult to associate with a particular heap or operation. Keep the shared
  fixture as the source of truth so variants cannot drift apart.
- Add deterministic seed cases when a newly discovered upstream regression is
  not already represented by the randomized operation fixtures.
- Consider optional serialization round-trip tests only if `serde` support is
  added. Java `Serializable` tests have no equivalent in the current API.
- Keep Java `null`-key tests out of scope: safe Rust keys cannot be null unless
  the caller explicitly chooses `Option<K>`, which is then a valid ordered key.
- Keep Java comparator-accessor and comparator-identity meld tests out of
  scope. Ordering is expressed by the key's `Ord` implementation, and heaps
  with different key types cannot be melded.
- Keep negative-capacity constructor tests out of scope. Rust capacities and
  branching factors use `usize`; invalid representable values such as zero and
  non-power-of-two d-ary degrees remain tested.
- Treat Java empty-heap exception tests as covered by Rust's `Option` contract:
  `peek` and `pop` return `None` rather than throwing.
- Keep iterator `remove` tests out of scope unless mutable heap iterators are
  introduced; Rust's current iterator APIs do not expose Java-style removal.
- Do not port `UnsignedUtils` unit tests directly. Radix behavior is tested
  through public `u32`, `u64`, `FiniteF64`, and `BigUint` heap APIs instead of
  Java compatibility helpers.

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
2. Design fallible heap traits and neutral shared error types.
3. Publish a migration guide and stabilize the resulting public API.
