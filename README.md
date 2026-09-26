# FlareML

**Model systems. Explore their executions. Find design bugs.**

FML is a finite systems modeling language with a native Rust model checker—not an application runtime. Actors represent participants: a sequential algorithm, event loop, service, thread, or computer. They do not imply a deployment technology or production conformance.

There is **one language and one execution contract**: finite identities, per-address FIFO mailboxes, atomic state-and-send turns, optional external inputs, and explicit weak scheduling fairness. There is no `semantics` selector or compatibility runtime.

## Try it

Use **stable Rust**. The CLI uses `clap`; no JVM, cloud account, or external checker is required.

```sh
cargo run --locked -- check examples/counter-replies.fml --trace-out /tmp/replies.json
cargo run --locked -- replay examples/counter-replies.fml /tmp/replies.json

# Intentional bug: a client waits forever for a reply that was never sent (exit 1).
cargo run --locked -- check examples/missing-reply.fml --trace-out /tmp/missing.json
cargo run --locked -- replay examples/missing-reply.fml /tmp/missing.json

cargo install --path . --locked
fml check --help
```

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

property "one increment stays bounded" { always (Counter.state <= 1) }
property "the increment is possible" { reachable (Counter.state == 1) }
property "submitted work finishes" {
  forall (i in inputs(Counter)) { i.submitted leads_to i.processed }
}

check OneIncrement {
  domain Int = 0..1
  mailbox_bound = 1
  inputs { once send(Counter, Increment) }
  fairness { weak runtime.progress }
}
```

- A singleton name is its address. `actor Account(id: AccountId)` creates one identity per finite key; use `Account.at(Alice)` and `Actor<Account>`. `Actor<T>` is a typed reference for sending messages, not a mutable state handle.
- `init` is pure. A stateful handler takes a state value and a message, and returns the next state. Stateless actors omit `init`, take only the message, and return `unit`.
- `send(address, message)` stages a one-way message. State and all outgoing messages commit together when the handler returns. Receivers can run only in later transitions. Replies require explicit protocol messages and reply addresses.
- Ordinary `let` functions describe local computation. Bindings and intermediate statements require `;`: `let next = state + 1; send(reply_to, Ack); next`. The final unterminated expression is the block's value; a trailing `;` discards it and returns `unit`. A non-tail `match` also requires `;`; match arms use `|`, with braces around multi-statement arms. Whitespace alone is not a statement separator. Exhaustive `match`, records, variants, `Option<T>`, and `Result<T, E>` describe finite data.
- Properties are read-only expressions. Local `let` bindings work in functions and handlers, not directly inside a property body; call a pure/specification helper when a predicate needs local bindings. Top-level `let` declares functions, not constants. A handler cannot inspect another participant's state or use observation views.
- `check` describes an experiment, not a `main()` function. `once send(...)` means **at most once**, not guaranteed arrival.

No suspended calls, threads, storage backends, crashes, retries, timers, imports, or dynamic spawning are built in. Model intervening steps explicitly: a read followed by a write must be two protocol turns if other participants can act between them. Atomic turns are modeling assumptions, not a guarantee made by an HTTP service or real transport.

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

P and Q are state predicates. Parenthesize compound predicates: `always (A.state == Done implies B.state)`.

Bare predicate properties are rejected. `reachable` is allowed only as the whole property body, over a state predicate. `exists` quantifies finite **data**, not executions. Temporal conjunction and stable `forall` are supported; arbitrary temporal nesting, temporal disjunction/negation, `next`, and strong fairness are not.

Safety is checked at initialization and as states are discovered, before graph closure. Reachability is `REACHED` or, only after complete exploration, `UNREACHABLE`. An unreachable query does not itself produce a failing exit code. Remaining temporal analysis requires a closed graph.

Without fairness, stuttering forever is allowed. `weak runtime.progress` prevents indefinite postponement of a continuously enabled mailbox. It does **not** force external submissions. Safety and reachability do not prune executions using fairness.

## Observing work

`inputs(A)` ranges over A's declared external slots across all keys. Each has `payload`, `target`, `submitted`, and `processed` fields. The flags are monotone; processing means its own handler committed, **not** that a reply arrived.

`messages(A)` includes generated messages through `message_bound = N` stable lifetime slots per actor declaration, across keys. Fields are `sent`, `processed`, `external`, `payload: Option<Message>`, and `target: Option<Actor<A>>`. Unsent slots have false flags and `None` data. Identical sends get different slots; slots never recycle.

The distinction between bounds matters:
- `mailbox_bound`: maximum **pending messages per address**, required.
- `message_bound`: maximum **lifetime observed sends per actor declaration**, optional.

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

Every `check` that successfully reads its source creates a unique `.fml/runs/<run-id>/` bundle:

```text
model.fml                 # exact source snapshot
configuration.json        # requested options, tool version, source SHA-256
report.json               # results, completeness/cutoff, witness file index
witnesses/0000.json        # each available counterexample/reached witness
```

Use `--artifacts-dir /path/to/runs` to change the parent directory. The location is printed on stderr and included in successful JSON reports as `artifacts_dir`. Invalid models and initialization errors also leave an error report. Unreadable source or unwritable storage cannot produce a complete bundle; persistence failures exit 4. `report.json` is written last; its absence means bundle creation did not finish. This is not a power-loss durability guarantee. Bundles contain model data; manage access and retention accordingly.

Verified properties need no witness; inconclusive runs do not fabricate evidence. `--trace-out` additionally exports one selected witness. Replay against the saved source, for example `fml replay .fml/runs/<run-id>/model.fml .fml/runs/<run-id>/witnesses/0000.json`. Reports are records of checker output, not independently checkable proof certificates.

Replay re-executes actions, compares snapshots and provenance, checks source identity, loop closure and fairness, and independently interprets the property on the trace. Only the current artifact format (**6**) is accepted; regenerate traces after source or format changes. Versioning artifacts does not select runtime behavior.

Reports support automatic color, `--color always|never|auto`, and `NO_COLOR`. JSON never contains presentation ANSI escapes. The full syntax, precedence, assumptions, limits, and deferred features are in [RFD0002](docs/rfds/RFD0002-functions-and-actors.md).

## Development and correctness

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

`rust-toolchain.toml` and normal CI use **stable**. Tests cover source-to-CLI behavior, atomicity, type/effect rejection, finite bounds, fair/unfair progress, missing replies, and trace corruption. Independent oracles compare:
- all two-state graph/predicate/fairness combinations for seven temporal patterns;
- generated three-state graphs with shared action identities;
- FIFO scheduler transitions and cutoffs;
- all 729 three-state/two-message deterministic transition tables;
- 160 source-to-verdict Boolean self-message cases using an independent orbit/cycle oracle, including optional input starvation and weak fairness.

Metamorphic regressions check actor renaming, declaration reordering, and persistence of concrete counterexamples under larger mailbox bounds. Inventory reservation and payment idempotency each have a failing model and an atomic-boundary repair, with reachable completion checks.

Like TLC, this is an explicit-state finite-model checker. It is not TLC, does not implement TLA+, and supports a much smaller temporal fragment.

These improve confidence; they are not a proof of checker correctness. Search uses exact state equality without symmetry, partial-order, or symbolic reduction.

**Nightly is optional and only for coverage/sanitizer-instrumented fuzzing:**

```sh
cargo install cargo-fuzz --locked
rustup toolchain install nightly --profile minimal
mkdir -p fuzz/corpus/source fuzz/corpus/trace_json
cp examples/*.fml fuzz/corpus/source/
cargo run --locked -- check examples/counter-replies.fml --trace-out fuzz/corpus/trace_json/replies.json
cargo +nightly fuzz run source -- -max_total_time=120
cargo +nightly fuzz run trace_json -- -max_total_time=120
```

A separate optional scheduled/manual workflow runs these campaigns and saves artifacts. See the [acceptance checklist](docs/rfds/RFD0002-implementation-checklist.md) for completed work and remaining validation.

The [proposed roadmap](docs/rfds/README.md#proposed-roadmap--not-implemented) sketches three sequential milestones: nondeterministic choice and faulty links, bounded dynamic spawning, then explicit suspension and reentrancy. These are designs for review, not currently supported syntax or engine behavior.
