# Feature request: finite maps, sets, and check-scoped identity pools

**Status:** draft; not filed. **Priority:** high for scaling examples beyond one key.

## Problem

Closed variants and keyed actors are useful for tiny examples, but tracking several independently created entities often requires a separate field or manually enumerated state for each key. General maps/sets and dynamic spawning are absent from the current language. Hand-enumeration makes it difficult to change a bound and compare the same property across checks.

## Minimal scenario

A service holds active entries keyed by an abstract `Key`; two clients may allocate, remove, and reuse keys. Check that each active key has at most one owner. Run checks with one, two, then three keys and two generations. Keep identities finite: no unbounded allocation is requested.

## Requested behavior

Consider finite `Map<Key, Value>` and `Set<Key>` values with explicit lookup, insert, remove and size/membership operations. Allow a check to choose a finite active identity pool (or provide a comparable reusable finite-domain encoding) without silently creating new addresses at runtime. Support quantification over the finite key domain and clear handling of absent entries. Report bounds and domain expansion in results and traces.

## Acceptance criteria

- One model handles multiple finite key-domain sizes without duplicating transition logic per key.
- Insert/delete/reinsert produces a replayable witness and preserves well-defined ownership semantics.
- Key or collection capacity exhaustion is **inconclusive**, not silent pruning or implicit eviction.
- Closed-variant models and eager keyed-actor initialization retain their existing meaning; type checking rejects invalid key/value combinations.

## Non-goals / design questions

No unbounded maps, arbitrary dynamic actor spawn, implicit garbage collection or hash-table performance promise. Decide separately how finite actor identities are allocated and whether all addresses are initialized eagerly; the collection proposal should not silently redefine actor lifecycle.
