# FlareML

**Model systems. Explore their possible executions. Find design bugs.**

FML is a standalone, Riot-inspired modeling language—not a language for deploying Workers. Its native Rust checker explores a finite state graph, checks invariants at semantic boundaries, and finds temporal counterexamples, including executions that repeat forever.

**Current status:** the first native vertical slice is implemented: legacy Workers + a primary-only D1 model, all seven temporal patterns below, weak progress fairness, source-mapped reports, and replay. The [`spike/actor-generalization`](docs/rfds/RFD0002-functions-and-actors.md) branch also implements typed functions and addresses, a synchronous experimental `actors-v1` profile, and a separate **experimental asynchronous `actors-v2` slice** with unified actors, FIFO mailboxes and one-way sends. This is **not yet the complete v0 resource set** from [RFD0001](docs/rfds/RFD0001-initial-language-and-model-checker.md) or all of RFD0002. Durable Objects, Queues, KV, buckets, Workflows, restarts, and D1 batches are currently rejected rather than approximated silently.

## Try it

Requires a current stable Rust toolchain. No Cloudflare account, JVM, or external model-checking process is needed.

```sh
cargo run -- check examples/login-fixed.fml
cargo run -- check examples/login-bug.fml --trace-out /tmp/login.trace.json
cargo run -- replay examples/login-bug.fml /tmp/login.trace.json
cargo run -- check examples/starvation.fml
cargo run -- check examples/lost-update.fml
# On the actor spike branch (the following two deliberately violate invariants):
cargo run -- check examples/actor-keyed.fml
cargo run -- check examples/actor-call.fml
cargo run -- check examples/actor-interleaving.fml
# The new async actor profile checks and replays a message-response protocol:
cargo run -- check examples/actor-messages.fml --trace-out /tmp/actor-messages.trace.json
cargo run -- replay examples/actor-messages.fml /tmp/actor-messages.trace.json
```

Buggy examples intentionally exit with code **1**. To install the CLI locally:

```sh
cargo install --path . --locked
fml check examples/login-fixed.fml
```

Reports use colors on supported terminals, respect `NO_COLOR`, and remain plain when piped. Override with `--color always` or `--color never`. `--format json` never includes presentation ANSI escapes.

```text
  ✗ VIOLATED  Login
  ────────────────────────────────────────────────────────

  ✗ invariant a non-existing user can't log in
    Counterexample · 4 steps
    initial AppDB.User = [User { id: Alice }]
     1. accept LoginAPI.handle_request(LoginRequest { user_id: Bob }) as request #1
     2. request #1 issues AppDB.User.get(Bob)
     3. request #1: AppDB.User.get completes with None
     4. request #1 responds Allowed
```

Actual reports also include file/line/column locations, covers, graph statistics, budgets, and the model's assumptions.

## Model shape

The complete [login model](examples/login-fixed.fml) combines typed data and resource declarations:

```fml
type UserId = Alice | Bob
type LoginRequest = LoginRequest { user_id: UserId }
type LoginReply = Allowed | Denied

d1 AppDB {
  table User {
    id: UserId primary_key
  }
}

worker LoginAPI {
  handle_request(request: LoginRequest): LoginReply {
    let user = AppDB.User.get(request.user_id);
    match user {
      | Some(_) -> respond(Allowed)
      | None -> respond(Denied)
    }
  }
}
```

Properties are pure observations—not scripts that perform operations:

```fml
invariant "a non-existing user can't log in" {
  forall (r in requests(LoginAPI.handle_request)) {
    (r.response == Some(Allowed)) implies
      AppDB.User.rows.contains_key(r.input.user_id)
  }
}

property "an accepted login eventually responds" {
  forall (r in requests(LoginAPI.handle_request)) {
    r.accepted leads_to r.completed
  }
}
```

A `check` chooses initial rows, finite input slots, domains, and scheduling assumptions. Each `once` input can arrive at most once, in any order; input arrival itself is not assumed fair.

```fml
check Login {
  semantics = "cf-core-v0"
  init { AppDB.User = [User { id: Alice }] }
  inputs {
    once LoginAPI.handle_request(LoginRequest { user_id: Alice })
    once LoginAPI.handle_request(LoginRequest { user_id: Bob })
  }
  fairness { weak runtime.progress }
}
```

`requests(...)` includes the finite input slots before acceptance and after completion. `accepted` means invocation start, not an HTTP acknowledgment. `response` is retained, so the login invariant compares past responses with **current** rows. A model with user deletion would need a different property if it only cares about existence at the authorization decision.

## What is checked

- **Invariants:** at the initial state and every reachable state, including between resource operations within a handler.
- **Covers:** whether a predicate can be reached, with a witness when it can.
- **Temporal properties:** universal claims over infinite behaviors. The native checker uses reachability and fair recurrent components, not long-running simulation.

Supported patterns, where `P` and `Q` are state predicates:

| Syntax | Meaning |
| --- | --- |
| `always P` | P holds at every state |
| `eventually P` | P holds now or later |
| `P leads_to Q` | Every P is followed by Q, possibly immediately |
| `P until Q` | P holds before the first Q; Q must occur |
| `always eventually P` | P occurs infinitely often |
| `eventually always P` | Eventually P stays true |
| `always (P implies always Q)` | Once P holds, Q holds from then on |

Finite `forall` and conjunction can combine temporal clauses. Arbitrary nesting, temporal disjunction/negation, `next`, and strong fairness are rejected. State predicates support Boolean operators, comparisons, finite quantifiers, and typed table/request views.

Without fairness, stuttering forever is a permitted behavior. `weak runtime.progress` excludes indefinite postponement of a particular continuously enabled continuation or operation completion. It does not make a false result true or guarantee environment inputs arrive.

A liveness failure includes a **lasso**: a finite prefix followed by a loop that can repeat forever. Replay re-executes every action, checks snapshots and loop closure, checks fairness, and independently evaluates the failed temporal formula on that trace.

## Scope and limits

```sh
fml check model.fml --check Login --property "an accepted login eventually responds"
fml check model.fml --max-states 100000 --max-depth 1000 --timeout 30s
fml check model.fml --format json --trace-out /tmp/counterexample.json
```

The defaults are 100,000 states, depth 1,000, and 30 seconds. These are **search budgets**, not definitions of `eventually`. Exceeding a budget yields an inconclusive result, never success. The report's `complete` field means the reachable **system graph** was completely explored; individual claim outcomes also account for temporal analysis cutoffs.

Closed variants provide finite domains. Stored `Int` and `String` values require explicit pools in the selected check:

```fml
domain Int = 0..3
domain String = ["alice", "bob"]
```

Ranges are inclusive; aliases share the underlying pool. Escaping a pool is inconclusive, not integer wrapping or a discarded transition. A successful check proves only the selected finite model under the printed assumptions—not all workloads or conformance of production code.

| Exit | Meaning |
| --- | --- |
| 0 | Selected claims verified in scope; covers have separate reached/unreachable outcomes |
| 1 | A design-property counterexample was found |
| 2 | Invalid/unsupported model or check configuration |
| 3 | Inconclusive exploration or domain cutoff |
| 4 | Tool, I/O, or replay validation error |

### Experimental asynchronous actors (`actors-v2`, spike branch)

The [message-response fixture](examples/actor-messages.fml) uses one `actor` declaration form, a pure `init(id): State`, `handle_message(state: State, message: Message): State`, and typed `send(address, message)`. Actors without `init` have a one-argument message handler returning `unit`. `once send(Actor.at(key), message)` is an optional external submission; only enabled mailbox-head processing is subject to `weak runtime.progress`. A handler runs to completion in one atomic transition: it returns the next owned state and publishes its staged outgoing sends together, in source order. Each typed actor address has a finite FIFO mailbox; `mailbox_bound = N` is required, and exceeding it is **INCONCLUSIVE**, never a silently dropped message. There is no implicit reply: pass a typed reply address and a correlation ID in the message. The fixture has a replayable cover, a safety invariant, and a format-version-4 trace.

This is a **fault-free, in-memory modeling profile**, not Cloudflare Queue/DO semantics: it has no crashes, retries, durability, I/O within callbacks, timeout, or live suspension. It has no `requests(...)` inspector for generated messages or dynamic temporal message quantification yet. More independent scheduler/fairness and capability validation is still required before RFD0002 is complete.

### Earlier synchronous actor profile (`actors-v1`, spike branch)

The following describes only the older `actors-v1` spike; it is **not** silently reinterpreted as the mailbox model in `actors-v2`.

Typed `let` functions bind to `stateless actor Name { method = function }` or `stateful actor Name(id: Key) { state: State = initial; method = function }`. State is modeled per finite key; `Name.at(key).state` inspects it only in specifications. Singleton stateful actors omit `(id: Key)`. Keyed input slots use `once Name.at(key).method(message)`. `Address<Name>` is typed, serializable model data produced by `Name.at(key)`, unlike the scoped `Actor<State>` owner capability; see [the address-routing example](examples/actor-address.fml). An effectful handler may directly bind `let reply = call(Name.at(key).method, message)` or `call(address.method, message)` (or `call(Name.method, message)` for a singleton/stateless actor). The caller suspends; the callee is independently accepted/scheduled; its committed state and reply precede caller resumption. Internal accept, resume, completion, and reply steps participate in `weak runtime.progress`; external input acceptance remains optional. `requests(Name.method)` ranges over **external** input slots only, not dynamically created calls. [The keyed fixture](examples/actor-keyed.fml) shows isolated accounts and [the interleaving fixture](examples/actor-interleaving.fml) shows a same-key lost update across an actor call.

This version assumes fault-free calls and retained modeled state, **not** durable storage, exactly-once queue delivery, timeouts, eviction, crash recovery, or a Cloudflare backend. Recursive actor calls are not mistaken for local recursion; their finite expansion hits an **inconclusive** frame limit rather than being pruned. Trace artifacts now use format version 3; earlier formats are rejected. `actors-v0` and the legacy `cf-core-v0` remain separate regression profiles; `worker` is still a transitional compatibility syntax. The unresolved design/adapter contracts are in [RFD0002](docs/rfds/RFD0002-functions-and-actors.md).

### Implemented resource semantics

Workers have concurrent invocation frames with no persistent Worker state. Resource calls split into issue and completion steps; a handler is not implicitly atomic.

D1 supports a typed table DSL, non-optional primary keys, single-column unique constraints, `Option<T>` fields, and key-based `get`, `insert`, `update`, and `delete`. A rejected mutation leaves rows unchanged. Multiple NULL/`None` entries are permitted for a nullable unique column. Reads and individual mutations are atomic at completion, but a read followed by an update is **not** a transaction—see [lost-update.fml](examples/lost-update.fml).

The current model excludes replicas, transport failures, crashes, unknown commit outcomes, and interactive transactions. Mutation `Result`s must be bound and immediately matched with `Ok` and `Err` arms. This intentionally restrictive rule prevents accidentally discarded errors while broader control-flow analysis is deferred.

## Implementation

The application-specific code is the language grammar, type/effect rules, semantic interpreter, temporal fragment, and witness logic. Reusable infrastructure comes from crates.io:

- `logos`: lexer generation
- `petgraph`: strongly connected components
- `clap` + `humantime`: CLI and duration parsing
- `serde` + `serde_json`: structured reports and replay artifacts
- `miette`: source diagnostics
- `owo-colors` + `supports-color`: terminal presentation
- `sha2`: source identity in replay artifacts
- `proptest` + `tempfile`: generative tests and isolated CLI tests

The checker uses BFS, exact state equality, deterministic edge ordering, and SCC-based liveness checks. It retains all edge labels, including fair self-edges. Symmetry, partial-order reduction, symbolic checking, and approximate visited sets are not enabled.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

Tests exercise the source-to-CLI pipeline, mutation boundaries, fairness, temporal boundaries, cutoff behavior, invalid programs, colors, and corrupt replay artifacts. A separate exponential oracle enumerates recurrent edge subsets of tiny graphs and cross-checks the native temporal algorithms; trace validation uses fixed-point temporal evaluation rather than SCC analysis.

Optional fuzzing uses `cargo-fuzz` and a nightly toolchain:

```sh
cargo +nightly fuzz run source
cargo +nightly fuzz run trace_json
```

See [RFD0001](docs/rfds/RFD0001-initial-language-and-model-checker.md) for the full design and remaining vertical additions. In particular, the Queue/DO slice and D1 batches are still required before declaring the planned v0 complete.
