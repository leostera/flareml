---
title: FML checks and bounds manual
description: "Finite experiments, deterministic setup, bounded actor populations, optional inputs, and exploration limits."
---

# FML checks and bounds manual

This page is bundled with your installed `fml` binary. Related: `fml skills actors` for setup and turns, `fml skills properties` for verdicts, and `fml skills cli` for exploration options and replay.

A `check` is a **finite experiment**, not a program entry function. It selects data pools, per-definition creation bounds, mailbox capacity, deterministic initial setup, optional external inputs, optional message history, and scheduling assumptions. Every check requires one `main { ... }` block and one `spawn_bound Type = N` for **every actor definition**, including unused definitions (use zero). Select a named experiment with `fml check model.fml --check Name` if necessary.

```fml
type Increment = Increment
actor Counter {
  init(): Int { 0 }
  handle_message(state: Int, message: Increment): Int { state + 1 }
}
property "increment is possible" {
  reachable (exists (counter in instances(Counter)) { counter.state == Some(1) })
}
check OneIncrement {
  domain Int = 0..1
  spawn_bound Counter = 1
  mailbox_bound = 1
  main {
    let counter = spawn(Counter);
    inputs { once send(counter, Increment) }
  }
  fairness { weak runtime.progress }
}
```

## Deterministic setup

`main` executes exactly once before exploration and property observation. It starts with no actor instances. Use `spawn(Type, args...)` to create the initial population and `send(reference, message);` to queue guaranteed initial work. `main {}` is valid and creates nothing. The return value is discarded; bindings are local, and setup is not a scheduler transition or fairness action.

Setup is deterministic: it may call pure helpers, spawn, send, and register external input slots. Direct or transitive `choose` and specification inspection are rejected. `inputs { once send(reference, payload) }` captures pure values and source locations but does not enqueue them. Each target must be an instance created during setup; registration does not force submission. Each captured slot may be submitted **at most once**, including never. An ordinary setup `send` is already in the initial mailbox and is not an external input.

No actor processes a message until the entire setup succeeds. Allocation, initialization, finite-value checking, or enqueue failure/cutoff exposes no partial initial state or zero-step witness. Setup bindings do not become global property variables; pass references through messages. Actors created later cannot register new external inputs.

## Finite data, actor, and mailbox bounds

`domain Int = 0..2` is an inclusive finite integer range. `domain Int = [-1, 0, 1]` and `domain String = ["a", "b"]` use literal lists. Int and String values need declared, nonempty pools when used in model data. A pool contains at most 1025 literals; it is a data domain, not a computation or a list of executions. Closed finite variants need no explicit domain. Out-of-pool evaluation is inconclusive, not wrapping or silent pruning.

`spawn_bound A = N` is an integer literal in `0..4096` for each actor definition. It caps the **lifetime** number of A instances created by setup and all handlers combined. Zero prohibits creation. Slots are allocated monotonically from zero per definition; instances are never deallocated and identities never reused. All created actors share the host address limit. A creation bound is not an application admission policy. Capacity exhaustion aborts the proposed initial setup or handler turn and yields an inconclusive result rather than silently rejecting a spawn or verifying a pruned graph.

`mailbox_bound = N` is required (1..4096) and limits pending messages **per created address**. It is not a lifetime send limit. Optional `message_bound = N` (1..4096) limits lifetime observed sends per actor definition across all instances when a property uses `messages(A)`. Setup sends count toward message history. Omit `message_bound` if lifetime observations are unnecessary.

## Scheduling and exploration limits

The checker always allows stuttering. Without fairness an enabled mailbox can be postponed forever. `fairness { weak runtime.progress }` prevents permanent starvation of a continuously enabled mailbox-processing action, but does not force input submission, actor creation, or a favorable `choose` branch. For safety and reachability, fairness does not remove reachable states.

The CLI has independent graph limits: `--max-states` (default 100000), `--max-depth` (default 1000), and `--timeout` (default 30s; cooperative). A state/depth/time, value-domain, evaluation, actor-creation, mailbox, or message-history cutoff is `INCONCLUSIVE`, never proof. Safety violations and reached witnesses already found remain valid; absence of a witness does not prove `UNREACHABLE` unless exploration closes. State search uses exact equality, without symmetry, partial-order, or symbolic reductions.

A check that successfully reads its source saves its snapshot, configuration, report, and available witnesses in `.fml/runs/<run-id>/` (or under `--artifacts-dir`). See `fml skills cli` for run bundles, results, and replay.
