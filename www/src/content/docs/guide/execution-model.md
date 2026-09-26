---
title: Execution model
description: How deterministic setup, bounded actor creation, scheduling, fairness, and replay define each FML check.
---

Each check runs one finite system under a single execution contract: deterministic setup, explicitly created actors, per-address FIFO mailboxes, atomic state-and-send turns, optional external inputs, and optional weak scheduling fairness.

## From no instances to an initial state

An `actor` declaration defines a participant type but creates nothing. Every selected check requires `spawn_bound Type = N` for every actor definition and exactly one `main { ... }` setup block. The checker starts with no instances, runs `main` once, then exposes the resulting initial population, mailboxes, and captured input slots.

```fml
type Increment = Increment
actor Counter {
  init(): Int { 0 }
  handle_message(state: Int, message: Increment): Int { state + 1 }
}
property "one increment is possible" {
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

`main` is deterministic setup, not a scheduled actor turn. It can spawn actors and queue guaranteed setup sends. An `inputs { once send(reference, message) }` block captures optional external slots; registering a slot does not enqueue it. `main {}` creates nothing. Setup bindings remain local, so pass actor references through messages when later turns need them.

## Transitions after setup

From the completed initial state, the checker may:

1. Stutter without changing state.
2. Submit one unsubmitted external input slot, appending its message to the target FIFO mailbox. Inputs are optional; submission does not run the handler.
3. Process the head message of any nonempty mailbox. The checker evaluates that handler against the actor's old state, then commits the dequeue, next state, new instances, and staged outgoing messages atomically.

The receiver processes a sent message only in a later transition. Self-sends also run later and see the newly committed state. Messages to one address remain FIFO; different actor instances can interleave.

## Bounded dynamic creation

Setup and handlers use `spawn(Type, args...)` to create fresh, typed actor references. Pure, typed arguments are passed to `init`; stateless definitions omit `init` and accept no arguments. A handler can retain or send a new reference during its turn, but the instance is not published and cannot run until that turn commits. Setup and handlers share each definition's `spawn_bound` lifetime pool. Identities are allocated monotonically per definition and never reused.

Every definition needs a bound, even if unused; `0` explicitly forbids creation. All created addresses share the host limit. Exhausting a creation, mailbox, value-domain, message-history, or search bound makes exploration inconclusive. The checker does not silently reject a spawn, partially commit earlier work in the same turn, drop a message, wrap a value, or prune the overflowing successor and call the remaining graph complete.

## Fairness and progress

Without fairness, stuttering forever is allowed and an enabled mailbox can be postponed forever. `fairness { weak runtime.progress }` rules out permanent postponement of a mailbox-processing action that remains continuously enabled. It does not require optional external inputs to be submitted, actor instances to be created, or a favorable handler-local `choose` outcome.

Safety and reachability use the full reachable graph; fairness does not prune their states. Temporal liveness claims are checked over fair infinite paths. See the shared [properties manual](/reference/properties/) for the supported temporal fragment and vacuity considerations.

## Population observations

In properties, `instances(A)` provides stable potential slots for A from the initial state. Each slot has `created` and `reference` fields; stateful actor slots also have `state`. Before creation, those fields are `false`/`None`; after creation, the reference stays stable and the state tracks committed turns. Universal properties range over unborn slots too, so guard state predicates with `created` or handle the optional state. A universal property over a zero-sized bound can be vacuous.

`inputs(A)` observes slots captured by setup and their submission/processing flags. Processing means that input's handler committed; it does not mean a reply arrived. `messages(A)` optionally observes lifetime sends across all instances of A using a finite `message_bound`. See [observations](/reference/observations/) for details.

## Traces and replay

A safety violation has a finite bad prefix; a reachability result has a finite witness; an infinite temporal counterexample is represented by a stem and repeating loop. `--trace-out` optionally exports one available witness. Every `fml check` that successfully reads its source also saves the exact source snapshot, configuration, report, and available witnesses under `.fml/runs/<run-id>/` by default.

`fml replay model.fml witness.json` reruns deterministic setup from an empty population, checks the completed initial snapshot, then re-executes the recorded actions and validates allocations, source identity, choices, state snapshots, provenance, loop closure, fairness, and property evidence. Replay the bundled witness against its bundled `model.fml` if the original has changed. See the [CLI, results, and replay manual](/reference/cli/).

## Scope

FML's execution engine does not inject network loss, duplication, crashes, restarts, timeouts, retries, persistence, timers, or external I/O. Represent selected behaviors explicitly with actors, inputs, and `choose`. Dynamic spawning is bounded and deterministic at each allocation; it is not OS process creation, supervision, or unbounded concurrency. Atomic state-and-outbox publication is an assumption of this model, not a guarantee made by a real network or database.

- [Actors and execution](/reference/actors/) describes identities, setup, state, messages, and turns.
- [Checks and bounds](/reference/checks/) describes finite populations, data pools, inputs, and capacity.
- [Observations](/reference/observations/) describes instance, input, and message views.
