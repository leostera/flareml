# RFD0004 — Explicit populations and bounded actor spawning

**Status:** implemented; stabilization and independent validation ongoing. Builds on [RFD0003 (choice)](RFD0003-nondeterministic-choice-and-faulty-links.md) and the [current RFD0002 contract](RFD0002-functions-and-actors.md). Finite testing is not a proof of correctness.

**Next:** [RFD0005 (suspension and reentrancy)](RFD0005-suspension-and-reentrancy.md) remains an unimplemented sketch. Setup and spawn do not introduce either feature.

## Decision: definitions never create instances

An `actor` declaration defines a participant type. A selected check's deterministic `main` explicitly constructs its initial population. The same `spawn` operation creates later instances during handlers. There is no `spawnable` modifier, implicit singleton, eagerly populated key domain, global reference named after a definition, or `.at(key)` constructor.

This is finite systems modeling, not an operating-system process launcher. Every handler remains one atomic, non-reentrant turn.

```fml
actor Coordinator {
  handle_message(message: unit): unit {
    let worker = spawn(Worker, false);
    send(worker, ());
  }
}
actor Worker {
  init(completed: Bool): Bool { completed }
  handle_message(state: Bool, message: unit): Bool { true }
}
property "two workers can be created" {
  reachable (forall (worker in instances(Worker)) { worker.created })
}
property "future workers finish" {
  forall (worker in instances(Worker)) {
    worker.created leads_to worker.state == Some(true)
  }
}
check TwoWorkers {
  spawn_bound Coordinator = 1
  spawn_bound Worker = 2
  mailbox_bound = 2
  main {
    let coordinator = spawn(Coordinator);
    inputs { once send(coordinator, ()) once send(coordinator, ()) }
  }
  fairness { weak runtime.progress }
}
```

## Deterministic setup

Every check requires exactly one `main { ... }` block. `main {}` is valid and creates nothing. Only the selected check's setup executes, once, before exploration or property observation. Its result is discarded; ordinary semicolon, lexical scope, and Result-handling rules still apply.

Setup uses the shared statement interpreter. It may call pure helpers, spawn, and send, including through effectful helpers. Direct and transitive `choose` and specification inspection are rejected. No actor processes a message until the whole setup succeeds.

- `let main = spawn(MainActor); send(main, Start);` creates an instance and enqueues startup work explicitly.
- `main { spawn(MainActor) }` also works: the unused reference is discarded, but the instance remains allocated. There is no implicit startup message.
- Setup bindings remain local. They are not global names accessible to handlers or properties. Pass references through initializer arguments and protocol messages; observe populations through `instances(Type)`.
- Conditional setup based on deterministic local values is allowed. Branch-local bindings do not escape.
- Failed allocation, initialization, value checking, or enqueueing exposes **no partial initial state**. Capacity/domain/work exhaustion is inconclusive, not verification, a modeled rejection, or a zero-step witness from partially completed setup.

### Optional external input registration

An `inputs { once send(reference, payload) ... }` statement is allowed only inside setup, including setup match arms. Targets and payloads are pure expressions evaluated once in the current local environment. Their values and source spans are captured as immutable external slots. Each target must be an instance created by the completed setup.

Registration does not enqueue anything. After setup, each slot may be submitted **at most once**, with no fairness requirement to submit. Captured references can also occur in payloads, including a participant's own reference. Repeated equal payloads still create distinct slots. An intermediate inputs block requires a trailing semicolon, like any other intermediate statement.

In contrast, an ordinary setup `send` is already enqueued in the initial state. With message history enabled it has a normal lifetime observation and `external == false`; only subsequent optional input submissions are external. Setup is not a scheduler transition or fairness action. Later-created actors receive work through modeled sends, not newly registered external input slots.

## Allocation expressions, initialization, and references

`spawn(Type, args...)` returns a fresh `Actor<Type>`. It must be a direct statement or the whole initializer of a local binding in setup, a handler, or an effectful helper. A discarded direct spawn is valid. The first operand is a definition name, not a reference; remaining operands are pure expressions matching the initializer's typed parameters.

Allocation effects are inferred transitively. Reject direct or indirect allocation in initializers, properties, finite domains, optional-input expressions, choice candidates, and other pure contexts. Calls to allocating helpers follow the same placement rules. No property inspector may also send, choose, or spawn.

An initializer is pure and returns the owned state. It may use typed arguments, including references already obtained by the creator, but cannot send, choose, spawn, or inspect state/observations. Its parameter list need not be an identity domain: `spawn(Worker, false)` simply passes a Boolean value. A definition without `init` is stateless and accepts no initializer arguments.

There is no automatic self binding, reference fabrication, asynchronous initialization, or implicit startup. To create a self-sending participant, bind its new reference and send it an explicit message containing that reference. A new binding is not visible while its own initializer is evaluated. State retained between turns is not durability.

## Finite identity pools

Every selected check declares `spawn_bound Type = N` for **every actor definition**, including unused definitions. N is an integer literal in **0..4096**. Repeated bounds, unknown definitions, and other values are invalid.

- Setup and handler creation consume the same lifetime pool.
- Slots are allocated monotonically from zero, independently per definition. Setup order and later interleavings determine allocation order, not random selection.
- Identities are opaque typed routing references, not model integers. They need no `Int` domain and support no application arithmetic index.
- There is no termination, deallocation, or identity reuse. Completed and unreferenced instances still consume capacity.
- All created instances share the 4096-address host guard. Potential unborn slots have no mailbox.
- Pool/host exhaustion is inconclusive unless a valid violation was already established. It never silently blocks creation, returns `None`, reuses a worker, or prunes a branch to prove verification.
- Application admission policies must be explicit model behavior, separate from checker bounds.

Zero is intentional: no instance may be created. A universal property over zero slots can be vacuous; a reachable universal predicate over an empty population is not evidence of creation. A deterministic allocator is not symmetry reduction or a theorem about larger populations.

## Atomic reservation and publication

For setup, start with an empty registry. For each local execution of a handler, start with the pre-turn registry and fresh branch-local reservations:

1. Derive the next slot from committed vector length plus this execution's earlier reservations for that definition. Check both capacities.
2. Evaluate pure initializer arguments and initialization; validate finite values. Retain the address, arguments, source/call context, and initial state locally. Stateless instances retain an internal unit value.
3. Subsequent statements can retain or transfer the reference and stage sends to it.
4. Only after successful execution, install every new registry entry and mailbox and append sends in source order. A handler also commits its next state, dequeues its head, and marks completion in that same transition.

No participant sees partial setup or partial handler publication. A new instance cannot process a staged message before commit. Failure in any initializer, value check, allocation, or enqueue abandons the proposed publication; earlier committed turns remain intact. Choice prefix re-execution resets reservations and cannot consume global slots. Any generated branch cutoff conservatively makes exploration incomplete; previously discovered genuine evidence remains valid.

The shared local evaluation and elaboration guards apply. Setup and handler execution, allocation, input capture, and publication poll the check deadline; timeouts are cooperative. No separate suspended-allocation interpreter is introduced.

## Registry, observations, and fairness

Exact state equality includes `spawned`, a vector of committed current values per definition. Vector length is the next identity, without a separate counter that could drift. It also includes per-address mailboxes and captured external slots and their flags. There is only one population representation, not static/keyed/dynamic registries.

`instances(Type)` is specification-only and ranges over all N stable potential slots, from the initial state onward:

| Field | Type | Before creation | After creation |
| --- | --- | --- | --- |
| `created` | `Bool` | false | true |
| `reference` | `Option<Actor<Type>>` | None | Some(reference) |
| `state` | `Option<StateType>` | None | Some(current state) |

Stateless definitions have no `state` field. Reference identity and creation are monotone; state may change. Match the optional state in a pure helper to inspect record fields. General state lookup through routing references is not introduced.

Stable temporal quantification includes unborn slots. `worker.created leads_to worker.state == Some(true)` therefore covers future workers rather than only the population present at expansion. Enumeration of data domains containing `Actor<Type>` is unsupported and errors explicitly; it must not manufacture references to unborn slots.

`inputs(Type)` includes all slots captured during setup for that definition, possibly none. `messages(Type)` uses `message_bound` lifetime slots across **all instances**, including setup sends; slots never recycle. `mailbox_bound` bounds pending messages per actual address.

FIFO and weak fairness attach to each created address. An unborn slot has no mailbox or enabled action. Processing any choice outcome services the same mailbox action. Fairness does not force optional submissions, creation, or favorable choices.

## Evidence and replay

Trace format is **8**, current-only. The first snapshot is the completed deterministic setup, including captured inputs, initial allocations, mailboxes, and message observations. Replay reruns `main` from an empty state and compares this entire snapshot before executing recorded actions. Setup is not fabricated as a fair processing action and has no choice transcript.

Traces include the selected `spawn_bounds`. Each handler action has an ordered `spawns` array containing `address`, allocating `span`, helper `calls`, pure initializer `arguments`, and `initial` state (unit when stateless). Replay constrains choices, re-executes initialization/allocation, and compares complete action metadata and successor state. Recorded addresses or initial values never drive the allocator.

Reject changed setup slots/payloads/targets, bounds, identities, order, arguments, initialization, allocation records, registry/mailbox entries, and source/call context. Semantically valid alternative traces are not inherently corruption; validation is not cryptographic authentication. Source snapshots and all available witnesses are saved in run bundles. Text replay shows initial populations, handler allocations, and state changes. Older artifacts must be regenerated.

## Validation

- All checked-in examples now use explicit setup and transferred references. Bug/repair verdicts and finite/lasso replay are exercised by `tests/examples.rs` and per-property tests.
- [Explicit startup](../../examples/explicit-startup.fml): setup-created actor and guaranteed initial enqueue, with no optional startup submission or implicit event.
- `tests/setup.rs`: empty populations, discarded spawn, captured optional inputs, guaranteed initial sends, deterministic/local setup, selected-check isolation, shared lifetime bounds, deadline/capacity failure, persisted inconclusive reports, old-syntax rejection, and initial-snapshot corruption.
- `tests/setup_oracle.rs`: 155 combinations of initial population size, Boolean initializer arguments, and creation bounds; independently expected registry values, reference identities, FIFO contents, captured inputs, observation provenance, and capacity failures.
- `tests/spawn_oracle.rs`: independent every-edge allocation/scheduler comparison for two coordinators, worker bounds 0..3, and both fairness settings, including lifetime exhaustion after completed work.
- `tests/spawning.rs`: fresh atomic publication, independent pools, choice/helper isolation, future-instance temporal claims, rollback, stateful/stateless workers, reference transfer/self-send, effect rejection, and shared setup/runtime host capacity.
- `tests/spawn_replay.rs`: altered allocation arguments/records, registry, bounds, choices, and CLI-persisted evidence.
- Independent mailbox, 729-table state-machine, graph/temporal, 160-case Boolean orbit, and Cartesian choice oracles are retained and migrated, not replaced by the implementation under test.

### Recorded validation and finite scopes

Current explicit-population revision: **174 tests passed** on stable, including all 17 examples and the executable README/RFD walkthroughs. Formatting, strict all-target Clippy, stable fuzz compilation, and diff whitespace checks passed.

Final local coverage/sanitizer smoke campaigns on `aarch64-apple-darwin`, nightly, seed `24680`, used `-runs=100000 -max_total_time=45 -timeout=10 -rss_limit_mb=2048`. Both reached the run cap successfully: **100,000 source executions in 9 seconds**, **100,000 trace-JSON executions in 3 seconds**, with no reported failures. Current source and format-8 witness seeds include explicit startup, correlated workers, choice/spawn, and faulty links. These are short smoke campaigns, not coverage-completeness evidence or independent review.

Updated measurements use the current explicit setup, default graph budgets, and the debug CLI. Times include CLI/artifact overhead and are single observations, not performance guarantees:

| Model | States | Edges | Outcome |
| --- | ---: | ---: | --- |
| Explicit startup | 2 | 3 | Verified in scope, complete |
| Correlated two-job example | 39 | 91 | Verified in scope, complete |
| Choice-and-spawn example | 8 | 16 | Verified in scope, complete |
| 1 independent coordinator | 4 | 7 | Verified, 16 ms |
| 2 independent coordinators | 16 | 40 | Verified, 7 ms |
| 3 independent coordinators | 64 | 208 | Verified, 12 ms |
| 4 independent coordinators | 256 | 1024 | Verified, 43 ms |
| 5 independent coordinators | 1024 | 4864 | Verified, 198 ms |
| 6 independent coordinators | 4096 | 22528 | Verified, 1015 ms |

The synthetic setup explicitly spawns N instances of one stateless coordinator definition and captures one optional unit input per instance. Each invocation creates one Boolean worker initialized false and sends it unit work; processing makes its state true. Both lifetime pools are N, mailbox capacity is 1, no lifetime message history is selected, and weak progress is declared. Requirements are `always true` and future-worker completion. The rapid growth is evidence of finite exploration cost, not a practical scalability claim.

## Deferred

Automatic self bindings, termination, identity reuse, supervision, migration, restart/durability, unbounded creation, shared-memory capabilities, suspension, reentrancy, and nondeterministic setup. None is implied by explicit population construction.
