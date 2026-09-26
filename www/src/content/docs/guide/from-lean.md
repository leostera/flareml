---
title: For Lean users
description: Use FlareML's finite-state exploration alongside Lean proofs, with a clear boundary between their guarantees.
---

Lean is a proof assistant: you state propositions and construct proof terms that Lean's kernel checks. FlareML is an explicit-state model checker for finite actor systems. They can complement each other, but a successful FML check is not a Lean theorem and does not produce a proof term.

## Different questions, different evidence

A Lean theorem can establish a proposition for every value covered by its assumptions, including parameterized or unbounded structures when the theorem and proof support that generality. An FML check explores the reachable graph of one finite model: its deterministic `main` setup, bounded actor creation, finite data pools, optional inputs, mailbox capacities, scheduling assumptions, and search budgets.

If exploration closes without cutoff, `always P` is verified over every reachable state of **that finite model**. A cutoff is inconclusive. Neither result alone establishes a theorem about all natural numbers, every queue size, an implementation, or a production service. FML counterexamples are executable traces that can be inspected and replayed; they are not kernel-checked proofs.

## Think in terms of refinement boundaries

Lean functions and inductive types may describe pure algorithms, data structures, or protocol states. To model a protocol in FML, decide which component owns each state value, what the setup creates, what messages cross between components, and where a step is atomic. Later instances can be created by handlers within per-definition lifetime bounds. FML handlers cannot directly inspect other actors' state; communication happens through messages processed in later turns. Its data and actor population must be finite for a check.

A useful workflow is:

1. Build a small FML model of the protocol boundary and ask whether a safety or progress property has a counterexample.
2. Use the trace to refine the protocol, identify a missing assumption, or formulate a stronger invariant.
3. State and prove a general theorem in Lean when the goal requires an unbounded guarantee.
4. Keep the correspondence explicit: ensure the Lean theorem and FML model describe the same transition boundaries and environmental assumptions.

The reverse direction is useful too: a proved lemma about a data structure may justify an abstraction in FML, but the FML result still depends on the actor model and finite scope.

## When to keep Lean in the loop

Use Lean when you need machine-checked derivations, inductive invariants over unbounded state, or proofs about implementation-independent mathematics. Use FML when an explicit finite protocol graph and concrete counterexample path are the faster way to explore interleavings. Neither replaces the other: a finite check can find a bug quickly, while a Lean proof can establish a broader theorem if its assumptions faithfully capture the system.

Start with [properties and verdicts](/reference/properties/) and [execution semantics](/guide/execution-model/) to understand exactly what an FML `VERIFIED` result covers.
