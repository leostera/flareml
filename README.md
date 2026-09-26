# FlareML

**Model systems. Explore their executions. Find design bugs.**

FML is a finite systems modeling language with a native Rust model checker—not an application runtime. Actors represent participants: a sequential algorithm, event loop, service, thread, or computer. They do not imply a deployment technology or production conformance.

There is **one language and one execution contract**: finite identities, per-address FIFO mailboxes, atomic state-and-send turns, optional external inputs, and explicit weak scheduling fairness. There is no `semantics` selector or compatibility runtime.

## Get started

Install the CLI from this checkout with **stable Rust**. No JVM, cloud account, or external checker is required.

```sh
cargo install --path . --locked
fml check examples/counter-replies.fml
# An intentional missing-reply bug produces a counterexample (exit 1):
fml check examples/missing-reply.fml
```

Each run saves its source, report, and available witnesses in `.fml/runs/`; see [results and replay](#results-limits-and-replay) below. For command options run `fml check --help`.

The installed binary also includes a version-matched agent manual:

```sh
fml skills                  # overview and topic index
fml skills syntax           # detailed language syntax
fml skills actors           # detailed actor semantics
fml skills --install        # optional: install all pages to ~/.agents/skills/flareml/
```

`fml skills` prints the agent-readable [overview](docs/skills/fml/SKILL.md); `fml skills syntax|actors|properties|checks|observations|cli` prints detailed standalone manuals. All pages are embedded at build time, so the installed binary works without the source tree. `fml skills --install` installs the overview and topic pages to `~/.agents/skills/flareml/`; it refuses to overwrite differing files. Use `fml skills --help` for the topic list.

[All examples](examples/README.md) have tested verdicts and replayable witnesses. Start with the [sequential workflow](examples/sequential-workflow.fml), then compare [lost updates](examples/lost-update.fml) with [atomic increments](examples/atomic-increments.fml).

## Language shape

```fml
type Message = Increment

actor Counter {
  init(): Int { 0 }
  handle_message(state: Int, message: Message): Int {
    let next = state + 1;
    next
  }
}

let bounded = (candidate: Option<Int>): Bool {
  match candidate { | None -> true | Some(value) -> value <= 1 }
}
property "one increment stays bounded" { always (forall (c in instances(Counter)) { bounded(c.state) }) }
property "the increment is possible" { reachable (exists (c in instances(Counter)) { c.state == Some(1) }) }
property "submitted work finishes" {
  forall (i in inputs(Counter)) { i.submitted leads_to i.processed }
}

check OneIncrement {
  domain Int = 0..1
  mailbox_bound = 1
  spawn_bound Counter = 1
  main {
    let counter = spawn(Counter);
    inputs { once send(counter, Increment) }
  }
  fairness { weak runtime.progress }
}
```

- `actor Counter` declares a type and creates nothing. `spawn(Counter)` returns a fresh `Actor<Counter>` reference, for routing messages—not accessing mutable state. There are no implicit singleton or keyed populations.
- `init` is pure; its typed arguments are supplied by `spawn(Type, args...)`. A stateful handler takes a state value and a message, and returns the next state. Stateless actors omit `init`, take only the message, and return `unit`.
- `send(address, message)` stages a one-way message. State and all outgoing messages commit together when the handler returns. Receivers can run only in later transitions. Replies require explicit protocol messages and reply addresses.
- Ordinary `let` functions describe local computation. Bindings and intermediate statements require `;`: `let next = state + 1; send(reply_to, Ack); next`. The final unterminated expression is the block's value; a trailing `;` discards it and returns `unit`. A non-tail `match` also requires `;`; match arms use `|`, with braces around multi-statement arms. Whitespace alone is not a statement separator. Exhaustive `match`, records, variants, `Option<T>`, and `Result<T, E>` describe finite data.
- Properties are read-only expressions. Local `let` bindings work in functions and handlers, not directly inside a property body; call a pure/specification helper when a predicate needs local bindings. Top-level `let` declares functions, not constants. A handler cannot inspect another participant's state or use observation views.
- Every `check` has a deterministic `main { ... }` setup block, run once before exploration. It creates the initial population and may enqueue messages. Its bindings remain local. Optional `inputs { once send(...) }` declarations inside `main` capture those references; each input is submitted **at most once**, never forced. `main {}` creates nothing.

No suspended calls, threads, storage backends, crashes, retries, timers, or imports are built in. Model intervening steps explicitly: a read followed by a write must be two protocol turns if other participants can act between them. Atomic turns are modeling assumptions, not a guarantee made by an HTTP service or real transport.

## Explicit nondeterminism

`let outcome = choose([Deliver, Drop, Duplicate]);` branches a handler turn over **every** listed outcome. It is not random sampling and does not suspend the handler. Use a nonempty literal list of compatible, pure candidate expressions; `choose` must be the whole initializer of a local binding. Handler-only helpers may choose transitively. Setup, initialization, properties, input declarations, and pure expression contexts cannot choose.

A link actor can choose to forward, drop, or duplicate a packet while the underlying engine mailboxes remain fault-free FIFO. See [loss](examples/faulty-link-loss.fml), [duplicate application](examples/faulty-link-duplicate-bug.fml), and its [idempotent repair](examples/faulty-link-duplicate-fixed.fml). Weak mailbox fairness does **not** force a favorable choice or eventual delivery. Reordering requires an explicit buffer, not just a different choice label.

Expansion is bounded: at most 128 encounters per turn and 4096 prefix executions, sharing the 100,000-entry evaluation budget across alternatives. Exceeding a guard is inconclusive, not permission to prune alternatives and verify the rest. [RFD0003](docs/rfds/RFD0003-nondeterministic-choice-and-faulty-links.md) specifies the execution and evidence contract.

## Bounded dynamic spawning

`actor Worker { ... }` defines a participant type. Both setup and handlers can use `let worker = spawn(Worker); send(worker, job);`. Pure initializer arguments are optional according to the declared `init` signature. Direct discarded `spawn(Worker);` is also allowed; it sends no implicit startup message. Allocation, initial state, and outgoing sends publish atomically at setup completion or handler commit. No participant runs midway through setup.

Every check supplies `spawn_bound Worker = N` (`0..4096`) for **each actor definition**, including unused definitions (use zero). Setup consumes the same lifetime pool as handler creation. Identities are never reused, even after work completes. Exhaustion is inconclusive, not modeled rejection or blocked creation. Identities are independent of `Int` domains.

In properties, `instances(Worker)` ranges over all N stable potential slots, including unborn ones. Fields are `created`, `reference: Option<Actor<Worker>>`, and, for stateful workers, `state: Option<StateType>`. Before creation these are false/None. This makes `forall (w in instances(Worker)) { w.created leads_to w.state == Some(true) }` meaningful for workers created later. External input slots capture references created during `main`. Later-created actors receive work through explicit sends. A definition name is not a reference, and setup bindings are not global property variables.

See [correlated jobs](examples/spawn-workers.fml), [choice plus spawn](examples/spawn-choice-workers.fml), and [RFD0004](docs/rfds/RFD0004-bounded-spawn.md). No termination, restart, suspension, reentrancy, or automatic self reference is implied. A creator can pass the new reference in an explicit message.

## One property declaration

| Form | Meaning | Evidence |
| --- | --- | --- |
| `always P` | P holds in every reachable state | A finite bad prefix on failure |
| `reachable P` | Some finite execution reaches P | A finite reached witness |
| `eventually P` | Every allowed execution eventually reaches P | A repeating counterexample on failure |
| `P leads_to Q` | Every P is eventually followed by Q | A repeating counterexample on failure |
| `P until Q` | P holds until Q; Q must occur | A bad prefix or repeating counterexample |
| `always eventually P` | P recurs infinitely often | A repeating counterexample |
| `eventually always P` | P eventually remains true | A repeating counterexample |
| `always (P implies always Q)` | After P, Q remains true | A finite bad prefix |

P and Q are state predicates. Parenthesize compound predicates: `always (finished() implies committed())`.

Bare predicate properties are rejected. `reachable` is allowed only as the whole property body, over a state predicate. `exists` quantifies finite **data**, not executions. Temporal conjunction and stable `forall` are supported; arbitrary temporal nesting, temporal disjunction/negation, `next`, and strong fairness are not.

Safety is checked at initialization and as states are discovered, before graph closure. Reachability is `REACHED` or, only after complete exploration, `UNREACHABLE`. An unreachable query does not itself produce a failing exit code. Remaining temporal analysis requires a closed graph.

Without fairness, stuttering forever is allowed. `weak runtime.progress` prevents indefinite postponement of a continuously enabled mailbox. It does **not** force external submissions. Safety and reachability do not prune executions using fairness.

## Observing work

`inputs(A)` ranges over the external slots registered in setup for instances of A. Each has `payload`, `target`, `submitted`, and `processed` fields. The flags are monotone; processing means its own handler committed, **not** that a reply arrived.

`messages(A)` includes generated messages through `message_bound = N` stable lifetime slots per actor definition, across all its instances. Fields are `sent`, `processed`, `external`, `payload: Option<Message>`, and `target: Option<Actor<A>>`. Unsent slots have false flags and `None` data. Identical sends get different slots; slots never recycle.

The distinction between bounds matters:
- `mailbox_bound`: maximum **pending messages per address**, required.
- `message_bound`: maximum **lifetime observed sends per actor definition**, including setup sends, optional.
- `spawn_bound A`: maximum **lifetime instances of A**, including setup creation, required per definition.

Exhaustion is inconclusive, not message loss or blocked sending. Omit lifetime history when checking infinite finite-state message cycles; with it, an unbounded sending protocol eventually exhausts the pool. Unused slots are reported separately from an empty temporal quantifier.

## Results, limits, and replay

```sh
fml check model.fml --check Scenario --property "submitted work finishes"
fml check model.fml --max-states 100000 --max-depth 1000 --timeout 30s
fml check model.fml --format json --trace-out /tmp/witness.json
fml replay model.fml /tmp/witness.json
```

| Exit | Meaning |
| --- | --- |
| 0 | Selected requirements verified in scope; reachability queries have separate outcomes |
| 1 | A requirement is violated, with evidence |
| 2 | Invalid or unsupported model/configuration |
| 3 | Inconclusive: a domain, capacity, evaluation, or exploration limit was reached |
| 4 | Tool, I/O, or replay validation error |

Closed variants are finite. `Int` and `String` data need explicit literal pools (`domain Int = 0..3`, `domain String = ["a", "b"]`). Crossing a bound never proves a property: values are not wrapped, transitions are not silently dropped, and full verification requires complete exploration.

Every `check` that successfully reads its source attempts to create a unique `.fml/runs/<run-id>/` bundle:

```text
model.fml                 # exact source snapshot
configuration.json        # selected check/property and exploration limits, tool version, source SHA-256
report.json               # results, completeness/cutoff, witness file index
witnesses/0000.json        # each available counterexample/reached witness
```

Use `--artifacts-dir /path/to/runs` to change the parent directory. The location is printed on stderr and included in successful JSON reports as `artifacts_dir`. Invalid models and initialization errors also leave an error report. Unreadable source or unwritable storage cannot produce a complete bundle; persistence failures exit 4. `report.json` is written last; its absence means bundle creation did not finish. This is not a power-loss durability guarantee. Bundles contain model data; manage access and retention accordingly.

Verified and unreachable properties have no witness; incomplete runs never fabricate evidence, but may retain a valid reached witness found before a cutoff. `--trace-out` additionally exports one selected witness. Replay against the saved source, for example `fml replay .fml/runs/<run-id>/model.fml .fml/runs/<run-id>/witnesses/0000.json`. Reports are records of checker output, not independently checkable proof certificates.

Replay reruns deterministic `main` and compares the complete initial snapshot, including captured inputs, before re-executing actions. It compares snapshots and provenance, checks source identity, loop closure and fairness, and independently interprets the property on the trace. Choice transcripts are replayed exactly, including encounter order, helper call sites, candidate positions, and values. Allocations are reconstructed and checked for freshness, initialization, bounds, and atomic publication. Only the current artifact format (**8**) is accepted; regenerate traces after source or format changes. Versioning artifacts does not select runtime behavior.

Reports support automatic color, `--color always|never|auto`, and `NO_COLOR`. JSON never contains presentation ANSI escapes. The full syntax, precedence, assumptions, limits, and deferred features are in [RFD0002](docs/rfds/RFD0002-functions-and-actors.md).

## Scope and further reading

Like TLC, FML explores a finite explicit state space; it does not implement TLA+ and supports a smaller temporal fragment. Search uses exact state equality without symmetry, partial-order, or symbolic reduction. A verified model is not proof of a deployed system's behavior.

The [language contract](docs/rfds/RFD0002-functions-and-actors.md), [choice extension](docs/rfds/RFD0003-nondeterministic-choice-and-faulty-links.md), and [explicit-population/spawn design](docs/rfds/RFD0004-bounded-spawn.md) describe the current assumptions and limits. The [RFD roadmap](docs/rfds/README.md) identifies suspension/reentrancy as an unimplemented sketch. For development setup, tests, fuzzing, and adding examples, see [CONTRIBUTING.md](CONTRIBUTING.md).
