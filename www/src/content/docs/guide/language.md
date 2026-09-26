---
title: Language reference
description: Complete reference for FML declarations, data, functions, actors, properties, checks, and observations.
---

FML files use five top-level declarations: `type`, `let` (a function), `actor`, `property`, and `check`. An actor declaration defines a participant type but creates no instances; each check explicitly builds a bounded population in deterministic `main` setup with `spawn`. There are no imports, namespaces, top-level constants, implicit singleton/keyed populations, unbounded spawning, legacy `invariant`/`cover`, or `semantics` selector.

The pages in this reference are the same manuals bundled with `fml skills`:

- [Syntax](/reference/syntax/) — types, functions, blocks, patterns, expressions, quantifiers, precedence, and common mistakes.
- [Actors](/reference/actors/) — actor identity, state, typed messages, turns, and modeling boundaries.
- [Properties](/reference/properties/) — safety, reachability, temporal forms, fairness, and result interpretation.
- [Checks](/reference/checks/) — data pools, optional inputs, bounds, scheduling assumptions, and exploration budgets.
- [Observations](/reference/observations/) — `inputs(A)`, `messages(A)`, stable identities, and non-vacuity.

Use `fml check model.fml` to type-check and explore a complete file. Unsupported syntax is rejected, not ignored. Each reference page is shared with the corresponding manual printed by `fml skills TOPIC`.

Coming from another formal-methods tool? See [FlareML for TLA+, Alloy, Quint, Z3, and Lean users](/guide/from-other-languages/) for the semantic differences and a practical translation checklist.
