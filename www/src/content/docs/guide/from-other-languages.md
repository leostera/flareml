---
title: Coming from other modeling tools
sidebar:
  order: 3
description: A practical guide to translating models from TLA+, Alloy, Quint, Z3, and Lean into FlareML.
---

FlareML is useful when you want to explore a **finite, message-driven system**: named actors, private state, typed FIFO mailboxes, optional external inputs, and explicit scheduling. It is not a general-purpose replacement for the tools below. This section explains which modeling instincts carry over, where the semantics differ, and when to keep using your existing tool.

## Pick the guide for your starting point

- [For TLA+ users](/guide/from-tla-plus/) — move from a user-defined next-state relation to scheduled actor turns.
- [For Alloy users](/guide/from-alloy/) — distinguish relational instance search from operational state exploration.
- [For Quint users](/guide/from-quint/) — map executable state machines onto explicit mailboxes and atomic handlers.
- [For Z3 users](/guide/from-z3/) — distinguish satisfying assignments from reachable executions.
- [For Lean users](/guide/from-lean/) — use finite exploration alongside theorem proving, without confusing their guarantees.

## The FlareML execution contract

A check starts with no actor instances and runs its deterministic `main { ... }` setup once. Setup creates the initial population with `spawn`, can enqueue guaranteed messages, and can capture optional, unsubmitted input slots. Each actor definition has a finite `spawn_bound`; handlers may create further instances within that shared lifetime bound. After setup, a transition submits one input, processes one mailbox head, or stutters. Each handler is an atomic state-and-outbox turn; sends and allocations become visible when that turn commits, and recipients process messages in later turns. Messages to one address are FIFO; different instances can interleave.

The model must declare finite data pools, a `spawn_bound` for every actor definition, and mailbox capacity. External inputs are registered in setup and are optional. Specifications can inspect stable `instances(A)` slots, including unborn slots, so universal claims must account for uncreated instances. Weak fairness, when enabled, prevents starvation of a continuously enabled mailbox-processing action, but does not force an external input, instance creation, or a favorable `choose` outcome. The engine does not inject network loss, duplication, retries, crashes, timers, or persistence; model only the failure behaviors you intend to study. See the [execution model](/guide/execution-model/) and [actor reference](/reference/actors/) for the full contract.

When exploration finishes without hitting a bound or resource limit, a result is exact **for that finite model and its assumptions**. If a limit prevents a complete conclusion, the result is inconclusive—not a proof obtained by silently dropping behavior. Counterexamples and witnesses can be replayed; see [CLI, results, and replay](/reference/cli/).

## A translation checklist

1. **Choose one question.** Start with one invariant, reachability query, or progress claim—not a whole codebase translation.
2. **Name the state and population.** Decide which actor type owns each piece of state, how setup creates the initial participants, and which messages cross participant boundaries.
3. **Choose turn boundaries.** An FML handler is atomic. Split a real operation into messages when another participant can interleave between its steps.
4. **Model the environment explicitly.** Declare finite `inputs`; remember each input slot may remain unsubmitted.
5. **Make scope and scheduling visible.** Choose finite data pools, mailbox bounds, and any fairness assumption. Explain why each is appropriate.
6. **Translate the property last.** FML supports a finite temporal fragment, not every formula or query from the source tool. Check the exact meaning and inspect/replay the resulting evidence.

The [link-shortener walkthrough](/guide/link-shortener/) is a small end-to-end FML model. The [faulty-link bug model](https://github.com/leostera/flareml/blob/main/examples/faulty-link-duplicate-bug.fml) and its [repair](https://github.com/leostera/flareml/blob/main/examples/faulty-link-duplicate-fixed.fml) show how to model selected delivery/drop/duplication outcomes explicitly and compare a buggy receiver with a repair.
