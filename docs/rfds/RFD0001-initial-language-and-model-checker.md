# RFD0001 - Initial language and model checker

- Feature Name: `flareml-core`
- Status: Draft
- Mode: Proposal
- Author: leostera, with AI assistance
- Start Date: 2026-09-25
- Updated: 2026-09-25
- Backend Decision: Native Rust checker selected
- Implementation: Initial Worker/D1 vertical slice; see [current support](../../README.md)
- Related proposal: [RFD0002](RFD0002-functions-and-actors.md) proposes a generic function/actor core in place of the product-named actor syntax below; this RFD still records the original proposal and checker/resource contracts.

## Summary

Build FlareML (FML), a Cloudflare-specific systems modeling language, and a Rust command-line tool named `fml`. FML borrows Riot's ML-shaped types, records, and pattern matching, adds first-class Cloudflare resource declarations, and describes system behavior through typed handlers. `fml check` elaborates a finite model, explores its reachable state graph, checks invariants at every state, checks a specified fragment of linear temporal logic over infinite behaviors, and reports replayable counterexamples. The first release is vertical: actual `.fml` source goes through parsing, typing, resource semantics, state exploration, temporal checking, and source-level diagnostics. It is not a syntax demonstration or a simulation presented as verification. Start with a native explicit-state checker and a deliberately restricted temporal fragment; do not implement arbitrary LTL or a deployment compiler in this release.

## Motivation

Cloudflare applications compose primitives with materially different semantics. A Worker invocation is not a persistent actor. A Durable Object owns state under a stable identity, but asynchronous external operations can permit interleaving. Queues can redeliver messages. Workers KV can return stale values, including cached absence. R2 binding operations have strong consistency. D1 has relational constraints and operation boundaries, with additional consistency considerations when using read replicas and sessions.

These differences are often hidden behind application functions or a generic storage interface. Tests usually exercise selected schedules, rather than all permitted interleavings within a finite model. We want to describe a design, ask questions about it, and receive a concrete explanation of a permitted execution that violates an expectation.

Examples:

- Can the API authorize a user who is absent from the authoritative user table?
- Can duplicate queue delivery charge for the same job twice?
- Can two requests both act on a stale local decision across an external call?
- Can accepted work remain pending forever, even under stated scheduling assumptions?
- Does a failed database operation leave a handler in a state with no way to finish?

FML is a model, not the Worker implementation. Its handler notation describes abstract computation and effects, not deployable JavaScript. Checking an FML model does not verify that separately written production code implements it.

### Repository baseline

At drafting time this repository has no implementation, build files, prior RFDs, or commits. This document introduces the first RFD convention under `docs/rfds/`. All syntax, paths, commands, and outputs below describe proposed contracts, not implemented behavior. Riot's syntax was inspected at commit `e8127a13f25853c68cb373f5b7b3d4820d467924`; FML is inspired by it, not source-compatible with it.

### Initial implementation status

The repository now has a working Rust `fml check` / `fml replay` pipeline for Workers and the primary-only D1 subset. It implements named invariants/covers, the seven temporal patterns specified below, weak runtime progress fairness, exact BFS exploration, fair-cycle counterexamples, JSON artifacts, and validated replay. Queue/DO semantics and D1 batches remain pending; this is not the complete v0 described by the release gates.

Following implementation review, reuse crates.io infrastructure: `logos` for lexing, `petgraph` for SCC decomposition, `clap`/`humantime` for CLI parsing, `serde`/`serde_json` for artifacts, `miette` for source diagnostics, `owo-colors`/`supports-color` for terminal output, and `proptest`/`tempfile` for testing. Keep custom implementation focused on FML semantics, the restricted temporal algorithms, and witness construction.

The initial slice restricts a mutation `Result` binding to an immediately following `match` with explicit `Ok`/`Err` arms. Nested constructor and record-constructor patterns are rejected for now. See the root README for runnable examples and current limitations; the remaining sections continue to specify the target design.

## Goals

- Make Cloudflare primitives part of the language, with distinct operations and state-transition semantics.
- Preserve the agreed surface: algebraic types, records, `match`, resource blocks, and handlers such as `handle_request(request: JobRequest) { send(JobEvents, Start(request.job_id)) }`.
- Describe D1 tables using a typed schema DSL, not embedded SQL strings.
- Support finite domains, initial conditions, environment inputs, nondeterministic scheduling, and explicit failure profiles.
- Provide named `invariant`, temporal `property`, and reachability `cover` declarations.
- Find safety failures along any explored path and temporal failures involving infinite repetition.
- Clearly distinguish a violation, a complete check of the scoped model, and incomplete exploration.
- Produce human-readable and machine-readable, source-mapped, replayable counterexamples.
- Deliver the first implementation through working end-to-end slices in Rust.

## Non-goals

- Executing or deploying applications, provisioning resources, or generating migrations in v0.
- Verifying arbitrary Rust, JavaScript, TypeScript, or Riot programs.
- Unbounded proofs, theorem proving, probabilistic reliability estimates, or latency/cost prediction.
- Full TLA+, full LTL, higher-order temporal logic, or arbitrary user-defined fairness in v0.
- A complete Cloudflare emulator, every product API, SQL parser, ORM, or browser simulator.
- Equating modeled success with production correctness, security, or implementation conformance.
- Hiding missing semantics behind a permissive stub or an unexplained default.

## Guide-level explanation

### Mental model

An FML model has five parts:

1. **Data:** finite values, protocols, and schemas.
2. **Resources:** named Cloudflare primitives and their typed bindings.
3. **Behavior:** handlers whose resource operations define scheduling and commit boundaries.
4. **Claims:** state invariants, temporal properties, and examples we want to reach.
5. **Check configuration:** initial state, input workload, semantic profile, finite domains, and fairness assumptions.

A handler resembles a small program because a model must describe behavior. It cannot access the host filesystem, network, clock, or arbitrary libraries. Resource operations are interpreted by the model engine. The checker chooses among every enabled modeled step; it does not simply execute the file once.

### First complete example: login

The following is a complete proposed v0 fixture, including an intentional bug. Finite variant IDs avoid needing a string domain declaration in this first example.

```fml
// examples/login-bug.fml

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
      | None -> respond(Allowed) // BUG: should be Denied
    }
  }
}

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

cover "an existing user can log in" {
  exists (r in requests(LoginAPI.handle_request)) {
    r.response == Some(Allowed) && r.input.user_id == Alice
  }
}

check Login {
  semantics = "cf-core-v0"

  init {
    AppDB.User = [User { id: Alice }]
  }

  inputs {
    once LoginAPI.handle_request(LoginRequest { user_id: Alice })
    once LoginAPI.handle_request(LoginRequest { user_id: Bob })
  }

  fairness {
    weak runtime.progress
  }
}
```

Each `once` declares a distinct input slot. Either input can arrive first; the requests and their resource operations can interleave. `requests(...)` is a finite, read-only view of these slots, including not-yet-accepted and completed requests. It is not an unbounded execution log. `accepted` and `completed` start false and remain true once reached; `response` starts `None` and is set by `respond`. In this example, acceptance means the environment has started the invocation, not that the server has sent an HTTP acknowledgment.

`AppDB.User.get` in a handler is a modeled D1 operation. `AppDB.User.rows` in a property is an instantaneous, read-only view of committed model state; it performs no database operation. This distinction prevents the checker from accidentally changing the system while checking it.

Illustrative use and output:

```console
$ fml check examples/login-bug.fml --check Login --trace-out login.trace.json
VIOLATED: a non-existing user can't log in

  initial: AppDB.User = { Alice }
  1. accept LoginAPI.handle_request({ user_id: Bob }) as request #1
  2. request #1 issues AppDB.User.get(Bob)
  3. AppDB.User.get(Bob) completes with None
  4. request #1 takes the None branch and responds Allowed

  failed predicate:
    AppDB.User.rows.contains_key(Bob) == false
  at: invariant "a non-existing user can't log in"
  related source: None -> respond(Allowed)

Scope: 2 user values, 2 one-shot requests
Semantics: cf-core-v0; primary-only D1; resource transport failures excluded
Replay: fml replay examples/login-bug.fml login.trace.json
```

The renderer attaches actual file/line/column spans; the display above is illustrative, not measured output. It says what failed, not that it has inferred a universally correct patch.

Change the `None` branch to `respond(Denied)`. A complete exploration must then report both the invariant and temporal property as holding **in this scoped model**, and provide a witness for the cover.

The invariant deliberately relates retained successful responses to the current user table. That is suitable here because the model never deletes users. With deletion, it would express the stronger requirement that previously authorized users remain present. A future observation/action-predicate facility can express existence specifically at the authorization decision. Do not silently interpret a state predicate as a historical assertion.

### Properties are claims, not test scripts

An invariant has no effects and returns a Boolean:

```fml
invariant "all sessions refer to a current user" {
  forall (session in AppDB.Session.rows) {
    AppDB.User.rows.contains_key(session.user_id)
  }
}
```

This example presupposes a declared `Session` table. `forall` quantifies over the specified finite collection, not all possible production users. Empty collections make `forall` true; covers help expose this kind of vacuity.

Temporal properties relate successive states:

```fml
property "a completed job stays complete" {
  forall (id in JobId) {
    always (Job(id).state == Complete implies
      always (Job(id).state == Complete))
  }
}
```

The persistence pattern above is supported explicitly in v0, as described below; arbitrary nested temporal expressions are not. `JobId` here denotes the finite domain of a declared type. `Job(id).state` is a property-only inspection, not an RPC.

The source cannot send messages inside an invariant to force the desired outcome. Inputs belong in `check`; effects belong in handlers; claims only observe.

### Cloudflare is in the vocabulary

These are distinct resource forms, not aliases for `store`:

```fml
kv UserCache : KV<UserId, UserProfile>
bucket Uploads : Bucket<ObjectKey, BlobTag>

d1 AppDB {
  table User {
    id: UserId primary_key
    email: String unique
    name: String
  }

  table Job {
    id: JobId primary_key
    user_id: Ref<User>
    state: JobState
    index(user_id)
  }
}
```

This inventory sketch assumes the named domain types exist. `BlobTag` represents an abstract payload identity, not all possible file bytes. A `check` must supply finite values for open-ended types such as `String`.

`kv`, `bucket`, and `workflow` are part of the planned language, but their operational support follows the first release. A v0 model using one receives an unsupported-feature diagnostic, not a successful check of an empty implementation. The same rule applies to deferred schema features such as `Ref` and `index`.

## Reference-level explanation

### Inventory and release boundary

| Area | First release | Subsequent vertical extensions |
| --- | --- | --- |
| Values | Bool, unit, aliases, finite variants, records, `Option`, `Result`, bounded integers/strings, finite collections | Recursive data with explicit depth bounds, broader generics |
| Expressions | Literals, names, field access, constructors, `let`, `match`, Boolean/comparison operators, checked integer addition/subtraction | Broader pure function library |
| Workers | Typed request and queue handlers, per-invocation locals, `respond`, direct modeled effects | Cron, WebSockets, service-binding RPC, external fetch |
| Durable Objects | Keyed state, exhaustive message handlers, `call`, atomic local `transition`, external-call suspension | Volatile fields/restart recovery, alarms, input/output-gate detail beyond the declared profile |
| Queues | Typed messages, Worker consumer binding, one-message attempts, duplicates, retry/ack/exhaustion states | Batching, retention clocks, delayed delivery, dead-letter queues |
| D1 | Table declarations, primary/unique constraints, key-based get/insert/update/delete, atomic batches, primary-only model | References, indexes, predicates/joins, replica sessions/bookmarks |
| KV | Reserved primitive; reject operational use in v0 | Cached values/absence, independent location views, refresh and expiry |
| R2 bucket | Reserved primitive; reject operational use in v0 | Object/metadata operations, conditional writes, strong consistency, separate cached access |
| Workflows | Reserved primitive; reject operational use in v0 | Durable steps/checkpoints, retry, sleep, external effects, recovery |
| Specifications | Initial state, finite one-shot inputs, state predicates, named invariants/covers, temporal patterns, weak action fairness | General temporal composition, strong fairness, action predicates, observation monitors |
| Checking | Exhaustive explicit-state search, exact deduplication, finite witnesses, fair lassos, truthful cutoffs, replay | POR, symmetry, symbolic backends, parallel search |

The first useful development slice is Worker + D1 + login; the first release also exercises Worker + Queue + Durable Object. No milestone may substitute handwritten Rust state machines for the public source-language acceptance tests.

### Rust implementation and module boundaries

Use Rust. Algebraic data types and exhaustive matching fit ASTs, typed operations, model values, and checker outcomes. Rust also supports a single distributable CLI, deterministic graph algorithms, profiling, and fuzz testing without a JVM or native FFI runtime requirement. C++ offers no identified simplification that justifies its additional memory-safety burden here.

Start with one Cargo package, a library, and the `fml` binary. Avoid a workspace of tiny crates until boundaries justify it. Proposed modules:

```text
src/
  syntax/       lexer, tokens, spans, parser, AST
  types/        name resolution, type/effect checking, finite-domain elaboration
  ir/           resources, handler control flow, operations, state predicates
  semantics/    worker, durable, queue, d1, environment, scheduler
  checker/      state interning, BFS, graph storage, temporal monitors, SCC analysis
  diagnostics/  source reports, text/JSON outcomes
  trace/        witness construction, format versioning, deterministic replay
  cli/          check/replay arguments, resource budgets, exit codes
examples/
tests/          parser, typing, semantics, temporal, CLI, replay, regression fixtures
```

Use recursive descent for declarations/statements and a Pratt parser for expressions. All AST and IR nodes retain source spans. No interpreter FFI, host-language `eval`, or arbitrary extension hooks. Choose ordinary maintained CLI/serialization/diagnostic crates during implementation and commit a lockfile; graph and semantic contracts must not depend on a particular package.

Pipeline:

```text
.fml source
  -> AST + spans
  -> resolved and typed model
  -> finite domains + check configuration
  -> resource-aware transition IR
  -> reachable graph + invariant/cover results
  -> temporal obligation graphs + fair-cycle analysis
  -> outcome + source-level witness + replay artifact
```

### Syntax and typing contract

FML uses `type X = A | B(T)` and `type R = R { field: T }`, braced `match`, `let`, optional semicolon statement separators, and `//` comments. Function-style primitive calls use parentheses, as in `send(JobEvents, Start(id))`. Handler parameters and return types are explicit; local types are inferred. Omitting a return type means `unit`. Parenthesized temporal operands are required when precedence could be ambiguous.

A parser specification and checked grammar fixtures must accompany implementation. In particular, newlines separate resource members; braces delimit branches/blocks; a trailing expression determines a pure block's value. Semicolons inside the source example are valid, not mandatory decoration.

Semantic checks include:

- Exhaustive variant matching, correct constructors, and no implicit coercions or unchecked `any`.
- Primitive-specific operations: no `bucket.transaction`, Queue `get`, or generic `store.write`.
- Queue payload and consumer parameter agreement; keyed DO reference and message protocol agreement.
- D1 row/column/key typing and schema validity.
- Boolean, effect-free invariant/cover bodies; temporal expressions only in properties.
- No resource-state inspection in executable handlers except through the resource's operations. A DO may inspect its own `state`; it may not inspect another object's state directly.
- No recursive functions or unbounded handler loops in v0. Handler control flow terminates between effects.
- Escaping a declared value domain is explicit and yields `INCONCLUSIVE` with the triggering operation, never host wraparound, implicit saturation, or a silently removed transition. Malformed expressions are static errors; reachable semantic faults are separately diagnosed.

Built-in `Option` and `Result` and resource parameterization do not require implementing full Hindley-Milner polymorphism in v0. Aliases are transparent; `type UserId = String` is not magically a nominal security boundary. Distinct ID variants can express separation now; nominal opaque types can follow.

An internal effect classification distinguishes pure evaluation, local state transitions, D1 operations, queue operations, and actor calls. Do not promise that the type system proves distributed correctness: constraints involving executions belong to checking.

### Finite scope and initialization

A `check Name { ... }` selects one model instance. Multiple named checks may share declarations. If a file has more than one check, the CLI requires `--check`; v0 does not silently select one.

- Closed variants are finite domains automatically.
- Open types require explicit pools, for example `domain String = ["alice", "bob"]` and `domain Int = 0..3`. Aliases share their underlying domain.
- Records and variant payloads range over finite products of their fields. Unused combinations need not be materialized eagerly.
- D1 tables initially contain the listed rows; omitted tables are empty. Invalid initial constraints are model-input errors.
- Each DO has a declared initial state for every key in its finite domain; absent implementation instances can be represented lazily without changing their property-visible initial state.
- Queue state starts empty unless explicit seed messages are supplied.
- Each `once` input is one possible invocation, enabled until accepted, with a stable slot ID. There are no invisible external inputs. Acceptance need not happen without an explicit environment fairness assumption.
- Queue/actor handlers can generate additional work; finite input alone does not guarantee bounded storage or concurrency.
- Queue checks explicitly scope transport duplication, for example `scope { extra_copies JobEvents = 1 }` permits zero or one additional transport copy per accepted logical message over its lifetime. This is a model restriction, not a Cloudflare guarantee. Failed-attempt retries remain separate. No duplicate scope is inferred from available memory.

Implementation capacities for active invocations, pending operations, queued entries, and table rows are guards against unbounded growth. Attempting to exceed one yields `INCONCLUSIVE` with a witness to the capacity boundary. It must not silently disable the operation, drop a message, or turn a partial exploration into a successful check. A named, user-specified bounded environment can intentionally restrict workloads, but reports must describe that restriction as model scope, not a production capacity guarantee.

### State and transition model

A state contains all information needed to determine future transitions and state predicates:

- Committed resource state: DO values, tables, queue message/attempt records.
- Active invocation IDs, handler program counters, live locals, and call/return correlations.
- Pending resource operations and completed-but-not-yet-consumed results.
- Input-slot acceptance/completion/response information.
- Finite retry counters and other profile state that affects future behavior.

No wall-clock timestamps, unbounded trace history, random seeds, or ever-growing debug IDs belong in state identity. Immutable schema/code and source maps live outside states. Temporal checking may add finite monitor state to a product graph; it must not merge histories whose outstanding obligations differ.

A labeled edge is one semantic step: accepting an input, executing a local segment, issuing/completing a resource operation, committing local state, delivering/acking a message, or applying a modeled fault. Edge labels preserve logical action identity and source provenance even when source and target state values are identical.

Pure evaluation is deterministic and runs to the next semantic boundary. Resource operations have explicit issue/completion boundaries; the requesting continuation suspends while an operation is pending. Another invocation can progress. Do not make an entire handler atomic by default.

Only primitive-defined transactions or local transitions are atomic. A D1 write followed by `send` is not one transaction. A DO update followed by a call to another resource is not one transaction. `atomic { arbitrary network work }` is not a language feature.

### Primitive semantics in v0

Every profile must document its state, enabled actions, operation results, commit point, cancellation behavior, scheduling boundaries, fairness action families, and omitted behaviors. The profile ID appears in results and traces. Semantic changes require a new profile version.

#### Worker

A Worker has concurrent invocation frames and no persistent per-Worker application state. Bindings are resolved from named resources used by its handlers; explicit capability lists are a possible later refinement. Locals survive a modeled suspension but not invocation failure. `respond(value)` completes a request and records its result in its input slot. Return from a queue handler requests successful acknowledgment unless it explicitly retries.

The base profile excludes unsolicited crashes and resource transport failures. They must not be added informally by a renderer or hidden behind an automatic retry. Queue-attempt failure is separately included in the queue profile below. Future timeout profiles must distinguish known non-commit from an unknown result after possible commit.

#### Durable Object

Proposed handler form:

```fml
durable Job(id: JobId) : JobEvent {
  state: JobState = Pending

  on Start {
    match state {
      | Pending -> transition Running
      | Running -> transition Running
      | Complete -> transition Complete
    }
  }
}
```

This fragment assumes the corresponding types, including a `JobEvent` protocol with the `Start` message, are declared. Typed actor calls use `call(Job(id), Start)`; the protocol annotation, message patterns, and explicit reply annotations determine the accepted inputs and return type. Missing handlers are typing errors for a declared protocol, not implicit message loss. A handler whose only action is `transition` returns unit.

`transition value` commits a replacement for this object's modeled persistent state. A straight-line local read/compute/transition segment is indivisible. An external resource operation opens an interleaving point, including for another invocation of the same DO. The model does not promise that a DO executes each entire handler serially.

The initial profile abstracts synchronous SQLite-backed local transitions with persistence complete at their commit point. It has no volatile application fields and does not model pending local-storage writes, detailed output-gate mechanics, or restart replay. This restricted abstraction must be reported. Broader gate/recovery support needs its own semantic profile and litmus tests; do not generalize v0 conclusions to arbitrary DO code.

#### Queue

Proposed binding form:

```fml
queue JobEvents : Queue<JobEvent> {
  delivery = at_least_once
  consumer = Processor.handle_message
  max_retries = 2
}
```

`send(JobEvents, event)` issues a queue submission and completes when the modeled queue accepts it. Producer success does not mean consumer completion. A Queue invokes its Worker consumer, not a DO directly; a consumer can `call` the DO.

The v0 profile uses single-message deliveries with arbitrary ordering. It keeps each accepted envelope's logical identity, immutable payload, acknowledgment state, and attempt state. Separate sends of identical payloads remain separate logical messages; retries and transport copies preserve the original identity. Code must still supply application idempotency keys where appropriate.

Model two distinct causes of repeated processing:

- **Failed attempts:** an attempt can fail between effect boundaries. Its local frame is discarded; effects already committed to D1/DOs remain. Already-issued remote operations are not rolled back or automatically canceled: they can still complete, with the abandoned caller's result discarded. Retry is eligible until the explicit retry limit; exhaustion moves the message to an observable failed terminal state in this profile.
- **Transport duplication:** a delivery can nondeterministically produce an additional in-flight copy with the same logical identity, up to the check's explicit `extra_copies` scope. Acknowledging one delivery does not erase an already in-flight copy. This abstraction admits duplicated processing independently of handler failure. Active copies also count toward checker capacity guards; unlike the semantic duplicate scope, exceeding a capacity guard is inconclusive.

Successful handler completion and acknowledgment are separate transitions, allowing a failure after an application effect but before acknowledgment. No global ordering or exactly-once application effect is inferred. `delivery = exactly_once` is rejected, rather than allowing a user to wish away Cloudflare delivery semantics.

Retention expiry, batches, and dead-letter routing are excluded from v0 and prominently reported. An unlimited or fair scheduler does not imply eventual *successful processing*: all allowed attempts might fail and exhaust. A completion property generally needs a failed terminal outcome or a stronger, separately stated model assumption. Transport duplication is an abstraction, not a claim about Cloudflare's internal implementation or duplication rate.

Before enabling this profile, add a transition table fixing retry-limit accounting, concurrent-copy eligibility, and which acknowledgments close which pending deliveries. Test those rules directly; the prose above does not license an implementation to silently choose FIFO or serialize all copies.

#### D1

D1 is a finite relation store, not an embedded SQLite process per explored state. The initial schema DSL supports fields, one primary key per table, and single-field unique constraints. Nullable fields use `Option<T>`; other fields are non-null. A primary key cannot be optional. Unique constraints compare non-null values; multiple `None` values are permitted, matching SQLite NULL uniqueness behavior. A table declaration introduces a row constructor and a typed key.

`get(key)` returns `Option<Row>`. `insert(row)`, `update(key, row)`, and `delete(key)` have typed `Result` outcomes, including constraint violations and missing rows where applicable. Constraints are checked at the operation commit boundary. A rejected write changes no committed data. Ignoring an unhandled error result is a typing error.

`batch AppDB { ... }` groups a finite list of D1 writes into one all-or-nothing commit, matching an abstract transactional batch. It does not permit interactive reads, arbitrary computation between remote statements, calls to another resource, or a transaction spanning databases. Failure rolls back all writes in the batch. There is no generic interactive `d1.transaction` API implied by this notation.

Operations complete atomically at their modeled commit/read point against one authoritative database state. v0 explicitly selects primary-only access and excludes replication, session bookmarks, transport failure, and unknown outcomes. Separate reads and writes can race. A check-then-insert is not atomic merely because both use D1.

`Ref<User>` and `index(...)` remain proposed extensions: references should introduce referential constraints with explicit delete behavior; indexes generally affect performance, not reachable logical states. Storing arbitrary ADTs also requires a separately specified encoding if deployment/schema generation is ever added. The abstract relational model does not claim that SQLite natively supports those types.

### Property language and meaning

Separate three expression levels internally:

- `Value<T>`: pure values derived from a state or bound finite value.
- `Predicate`: a pure Boolean on one state.
- `Temporal`: a claim about an infinite sequence of states.

An `invariant "name" { P }` implicitly means `always P`. Check it on every initial state and every reachable successor, not just after a handler, at terminal states, or along the first path found. Deduplicating a state is safe only when the property is state-based or the necessary monitor state is included. Fairness does not prune safety exploration: even an unfair finite prefix can contain a real invariant violation.

The v0 temporal grammar intentionally supports the following patterns. Here `P` and `Q` must be state predicates, not arbitrary temporal formulas:

| FML | LTL meaning | Failure witness |
| --- | --- | --- |
| `always P` | `G P` | Reachable state with `not P` |
| `eventually P` | `F P` | Infinite behavior avoiding P from the initial state |
| `P leads_to Q` | `G(P -> F Q)` | Reach P and then avoid Q forever |
| `P until Q` | Strong `P U Q` | P fails before Q, or Q never occurs |
| `always eventually P` | `G F P` | Reach a suffix that avoids P forever |
| `eventually always P` | `F G P` | An infinite behavior with `not P` recurring |
| `always (P implies always Q)` | `G(P -> G Q)` | Reach P, then a state where Q is false |

`eventually` includes the current state; `until` is strong, so Q must eventually occur. A response obligation is already satisfied when P and Q hold together. `until` imposes no P requirement at the first state where Q holds.

Finite `forall` and conjunction may combine temporal clauses. A quantified variable remains bound to the same finite value across the entire clause. In v0, temporal quantifiers range over stable domains or input slots, not changing table membership; use state-level quantifiers for dynamic collections. Disjunction/negation of temporal clauses, strong fairness, `next`, and arbitrary nesting receive precise unsupported-fragment errors. Boolean connectives inside state predicates remain unrestricted. This is TLA+-inspired temporal specification, not compatibility with TLA+'s full action language.

`next` is intentionally absent because the number of internal effect steps is an abstraction boundary. It would also constrain future stuttering-preserving reductions.

A `cover "name" { P }` asks whether a state satisfying P is reachable and returns a finite witness. An unreachable cover is a reachability finding, not a safety violation. A cover stops being conclusively unreachable if exploration is cut short. Reports flag unreached response antecedents and universally quantified empty domains; a vacuous property is not silently advertised as a useful guarantee.

### Infinite behaviors, fairness, and deadlock

Finite graphs can describe infinite executions through cycles. A temporal checker must therefore search cycles, not declare success because a simulation ran for a large number of steps.

Add a stuttering self-loop to every state. This represents postponing progress and gives terminal states an infinite interpretation. There is no fairness by default. Consequently a completion property may fail because a ready request is never resumed. With `weak runtime.progress`, each instantiated ready continuation or pending operation completion is weakly fair: if that *specific action* remains enabled continuously, it must eventually execute. Fairness is not merely "some Worker runs"; that would still allow an individual request to starve.

Named fairness families expand to a finite set of action identities derived from resource/slot/operation IDs, not growing trace sequence numbers. Queue profiles also expose individually identified delivery and acknowledgment actions. A fair action guarantees execution, not a favorable nondeterministic result. Weak fairness does not require an intermittently enabled action to occur; strong fairness is deferred.

Operationally, weak fairness for action A is `G F (not enabled(A) or taken(A))`. `enabled` is computed in the original transition graph, never after restricting the graph to search for a property violation. `taken` is an edge label, including self-edges whose data state does not change. No outgoing edge may disappear because the checker deduplicated its destination.

A pending invocation with no productive enabled action is an unexpected deadlock and gets a finite diagnostic. A state with no pending work and only unaccepted optional environment inputs is quiescent, not an error. Stutter edges do not count as productive actions or hide deadlocks. Terminal states that violate an eventuality yield a lasso with a terminal self-loop.

Fairness is a liveness assumption, not an implementation fix. A report must show whether its lasso is fair and which assumptions excluded starvation. If no fair behavior exists under a check's assumptions, report an invalid/vacuous assumption configuration rather than claiming that all temporal properties were verified.

### Native checking algorithms

1. **Elaborate:** validate types, initialization, finite domains, semantic profiles, and property-fragment membership. No execution begins for an invalid model.
2. **Explore:** perform deterministic-order breadth-first search from all initial states. Check every invariant before inserting each state; record cover witnesses and predecessor edges. Intern states with exact structural equality and store all labeled graph edges, not just BFS-tree edges.
3. **Stop honestly:** a violation is a valid result even if other exploration remains. If a budget/capacity is reached without a violation, mark uncompleted obligations inconclusive. Do not manufacture terminal loops at unexplored frontier states.
4. **Build temporal obligation graphs:** share the system graph, adding small monitor states for patterns requiring history. For example, persistence records that P has occurred; response checking can select a P-without-Q state and search for a suffix containing no Q.
5. **Analyze recurrence:** use strongly connected components in the appropriate reachable subgraphs. A recurrent SCC must contain a real cycle (multiple vertices or a self-loop). For each weak fairness action, require a recurrent disabled vertex or an internal edge taking the action. Additional recurrence predicates must also be visited. Strong connectivity lets a witness closed walk visit all these obligations; an arbitrary simple cycle from that SCC may not do so.
6. **Construct and replay witnesses:** build an initial prefix and, for temporal failures, a closed walk satisfying the violation and fairness conditions. Validate every chosen edge with the semantic engine before reporting the trace.

Pattern-specific details:

- `F P`: search for a fair cycle reachable from an initial state entirely through `not P` states.
- `P leads_to Q`: find a reachable `P && not Q` state, then a fair cycle reachable entirely within `not Q`. The prefix to the trigger is unrestricted.
- `P U Q`: search prefixes before the first Q for `not P && not Q`, and search for fair infinite paths that avoid Q. A Q at the initial state satisfies the formula immediately.
- `G F P`: search for a reachable fair recurrent suffix entirely in `not P`; the prefix is unrestricted.
- `F G P`: find a reachable fair recurrent SCC with a `not P` vertex that the witness revisits infinitely often.
- `G(P -> G Q)`: a finite monitor bit records a trigger and checks Q in that state and every subsequent state.

Finite bad prefixes for safety-shaped temporal clauses are reported conservatively without using fairness to hide them, like named invariants. This policy is explicit: safety claims apply to all reachable executions; liveness claims are evaluated under declared fairness. Reports distinguish that safety interpretation from a general LTL implication with arbitrary assumptions.

BFS yields a shortest safety witness in semantic transition count when no reduction is active. Liveness witnesses are valid deterministic lassos, not promised globally shortest. A repeated model state on a random trace is insufficient evidence: its monitor obligations and fairness must also match.

### Minimizing state space without weakening claims

There are three different mechanisms; they must not be conflated:

1. **Model scope:** a user chooses representative IDs, payload abstractions, workload, and fault profile. Results concern that model only. Two users are not a theorem about any number of users.
2. **Exact reductions:** preserve behaviors relevant to a property under stated conditions.
3. **Search budgets:** stop exploration and produce inconclusive results if completeness was not reached.

v0 uses only straightforward exact techniques:

- Canonical ordering for tables/maps and true unordered queue collections; preserve message multiplicity, envelope identity, and all observable ordering.
- State interning with equality checks, not hash-only approximate visited sets or Bloom filters.
- Intern immutable values and share schema/code outside states.
- Drop dead local variables only after control-flow liveness analysis proves no future behavior or predicate can depend on them; initially retaining them is acceptable.
- Run deterministic pure code to the next semantic boundary instead of creating a state for every arithmetic subexpression.
- Store trace metadata/predecessors outside semantic state; reuse finite slots only when no state, monitor, message, or property can reference the old identity.

Measure states, labeled edges, peak frontier, bytes retained, and per-resource branching before adding advanced reductions.

Deferred reductions require separate correctness work and differential tests against the unreduced engine:

- **Symmetry:** only for declared interchangeable IDs; preserve distinguished constants, property bindings, and fairness identities. Alice and Bob are not symmetric in the login example because only Alice initially exists.
- **Partial-order reduction:** prove action independence, property invisibility, and cycle/fairness provisos. Independent-looking operations cannot be freely reordered for all temporal properties.
- **Property slicing:** preserve enabledness, synchronization, errors, and fairness as well as values directly mentioned in the property.
- **Time abstraction:** model timer order/regions rather than milliseconds, with explicit assumptions; timers are not in v0.

A counterexample produced under an abstraction is an execution of the model. It may motivate refinement rather than demonstrate a feasible production failure. Report abstraction choices instead of declaring every abstract trace a proven implementation bug.

### CLI, outcomes, and trace contract

Required commands:

```console
fml check model.fml
fml check model.fml --check Login --property "an accepted login eventually responds"
fml check model.fml --format json --trace-out failure.trace.json
fml check model.fml --max-states 100000 --max-depth 100 --timeout 30s
fml replay model.fml failure.trace.json
```

Depth is a search budget, not the definition of `eventually`. CLI defaults must be finite operational safeguards and appear in output; their exact values are implementation-time tuning. `--property` selects an obligation explicitly and reports what was not checked. When checking everything, return a result for every requested obligation, including not evaluated/inconclusive after early failure. A completed invariant check may coexist with an incomplete temporal analysis.

| Outcome | Meaning | Exit |
| --- | --- | --- |
| `VERIFIED_IN_SCOPE` | Every selected claim checked completely and held for the stated finite model/profile/assumptions | 0 |
| `VIOLATED` | Valid finite counterexample, fair lasso, or unexpected modeled deadlock found | 1 |
| `INVALID_MODEL` | Syntax/type/schema/profile/property-fragment error or inconsistent initial assumptions | 2 |
| `INCONCLUSIVE` | Budget, representational capacity, interruption, or incomplete analysis prevents a conclusion | 3 |
| `TOOL_ERROR` | I/O, unsupported artifact version, or internal checker/replay failure | 4 |

Finding a violation dominates aggregate status; an invalid model prevents checking. Unreached covers are reported separately and make no safety claim; required covers can be a later CLI policy. An aborted invocation need not return an exit code after an uncatchable process kill, but must never have already printed a success result.

Human output uses colored verdicts, source locations, state changes, and loop markers on supported terminals. It respects `NO_COLOR`, supports `--color auto|always|never`, and emits no presentation escapes in JSON. Human output includes the named claim, spans, scope, profile exclusions, fairness, reduction mode, exploration counts, and a causal trace. Trace steps show operation inputs/results, state diffs, selected nondeterministic outcomes, and links between sends, deliveries, calls, and responses. A lasso clearly identifies its loop start and why repetition violates the property. Explanations come from evaluated predicates and transitions, not an ungrounded generated diagnosis.

For example, removing fairness from the corrected login model permits this illustrative result:

```text
VIOLATED: an accepted login eventually responds

1. accept LoginAPI.handle_request({ user_id: Bob }) as request #1
2. LOOP START: request #1 is ready, accepted = true, completed = false
3. stutter; return to step 2 forever

The response obligation for request #1 remains unsatisfied.
Fairness: none. This is scheduler starvation, not a failed database read.
```

Adding weak progress fairness excludes this particular loop. It does not exclude a genuine deadlock, since no productive continuation is continuously enabled there. Reports must distinguish these explanations.

The versioned JSON artifact contains:

- Tool/format/semantic-profile versions and source/normalized-model hashes.
- Selected check/claims, finite domains, scope, and explicit assumptions.
- Initial state and stable transition choices with action IDs and source spans.
- State digests plus sufficient values/diffs for inspection.
- Failed predicate bindings, or temporal obligation and loop-start index.
- Fairness evidence and completeness/cutoff information.

Replay checks compatibility, rebuilds the model, and re-executes each transition choice. It validates guards, outcomes, state equality, the violated claim, and fairness/closure of any lasso. It does not merely print serialized descriptions. A mismatched model is refused; replay never silently falls back to a fresh search.

For covers, use the same witness machinery with an explicitly labeled reachability artifact. Later minimization may remove irrelevant interleavings, but must replay the minimized trace and preserve property and fairness evidence. Do not promise a minimal causal explanation just because BFS found a shortest prefix.

### Security, privacy, and observability

Checking is offline and needs no Cloudflare credentials. Model code cannot execute arbitrary host code. Source files and trace artifacts are untrusted inputs: enforce parsing/depth/allocation limits, validate artifact lengths and indices, avoid recursive graph traversals that can exhaust the stack, and handle budget exhaustion without corrupting results.

Traces may contain sensitive values placed in the model. Use synthetic domains, avoid production payload imports, and do not upload artifacts or telemetry by default. Source snippets and values in terminal output must be escaped. A redacted trace is a separate presentation artifact and may not be replayable; never mislabel it as the full replay record.

Provide local exploration statistics and optional debug logging without placing those counters into model state. Report internal assertions as tool failures, not design violations.

### Compatibility and migration

There is no existing implementation to migrate. Assign explicit language, semantic-profile, and trace-format versions from the start. A future parser change can include a migration diagnostic; a semantic change must not silently reinterpret an old passing check. Save assumptions and normalized model identity with every CI result intended for comparison.

The RFD's syntax is proposed, not frozen before implementation review. Any change to its meaning or acceptance examples must update this document. Later implementation tracking may use issues; the RFD remains the design contract, not a mutable completion checklist.

## Implementation and validation plan

Each milestone ends with `cargo run -- check` accepting real `.fml` source and emitting a meaningful result. Internal infrastructure alone is not a milestone. No public v0 release is complete before temporal checking, fairness, replay, and all release acceptance cases work.

### 1. Login safety, end to end

Build the Rust CLI, parser/spans, finite ADTs/records, basic type checking, D1 schema/get semantics, Worker frames, input slots, property inspectors, BFS, named invariants, and a source-level finite trace.

Acceptance: the safety-only login fixture fails with Bob, its corrected version passes in scope, and an initial-state invariant violation is also detected. At this development stage, unsupported temporal declarations must be rejected; the full guide fixture is not claimed supported until milestone 2. A reduced safety-only fixture exercises milestone 1.

### 2. Temporal checking through the same source pipeline

Add the defined temporal fragment, graph edge retention, weak action fairness, obligation monitors, SCC analysis, deadlock/quiescence classification, and lasso reports. Complete the full login fixture, including its response property and cover.

Acceptance: an unfair scheduler produces a starvation lasso without fairness; the corrected finite login model passes with weak progress fairness. A permanently stuck handler still fails under fairness. A terminal state with an unfulfilled eventuality fails. A short search budget is inconclusive, never evidence that an eventuality is false or true.

### 3. Queue/DO distributed safety, end to end

Add typed actor identity and calls, local durable transitions, consumer binding, queue delivery/ack/retry/duplication semantics, and D1 mutation/batch semantics. Add complete `.fml` fixtures for duplicate processing and read/act races.

Acceptance: a non-idempotent consumer violates a named property when the checker chooses duplicate delivery; an idempotent DO transition passes under the same explicit finite duplicate/input scope, with enough representational capacity for full exploration. A two-resource write/send sequence admits an intermediate state. Two invocations can interleave across an external effect in one DO; straight-line local transitions remain indivisible. Retry exhaustion is visible and cannot be hidden by a success-only liveness property.

### 4. Reproducible verification and release hardening

Add stable text/JSON outcomes, replay, exact budget accounting, scope/profile reporting, diagnostic snapshots, temporal vacuity reporting, and regression/differential suites. Profile and optimize only after preserving reference behavior.

Release gates:

- All guide-level v0 examples parse/type-check; all intentionally deferred sketches are documented as such and reject unsupported operational use.
- Every failed safety fixture has a replayable finite witness; every failed liveness fixture has a replayable fair lasso where fairness is declared.
- Invariants are evaluated at initialization and all semantic boundaries, including intermediate states within handlers.
- `always`, `eventually`, `leads_to`, strong `until`, recurrence, stabilization, and persistence each have passing, failing, and boundary fixtures.
- Changing a cutoff cannot turn an incompletely explored model into `VERIFIED_IN_SCOPE`.
- Exact state equality and graph edges survive deliberately colliding hash tests and multiple labels to one successor.
- An independently specified oracle agrees on a corpus of small transition systems and temporal patterns. TLC models or an independent LTL tool may be used in development/CI without becoming runtime dependencies. Include unfair/fair, transiently enabled, permanently disabled, and intermittently enabled action cases.
- A replay mismatch, corrupted artifact, or engine bug is never reported as a verified model or a user design violation.
- Rust formatting, linting, tests, and CLI acceptance tests run in CI; parser/trace fuzz targets exist.

Proposed validation commands once implemented:

```console
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -- check examples/login-bug.fml --trace-out login.trace.json
cargo run -- replay examples/login-bug.fml login.trace.json
cargo run -- check examples/login-fixed.fml
cargo run -- check examples/queue-duplicate-bug.fml
cargo run -- check examples/queue-idempotent.fml
cargo run -- check examples/starvation.fml
```

Commands for buggy fixtures intentionally exit nonzero and must be asserted accordingly by the acceptance harness. At initial drafting there was no Cargo project. The implemented Worker/D1 slice now runs the Rust checks and CLI tests described in the [README](../../README.md); the Queue/DO commands remain future acceptance targets.

## Drawbacks

- A model can be faithfully checked and still omit the production failure that matters. Profile exclusions and implementation/model drift remain significant risks.
- State explosion remains even with finite values. This proposal prioritizes transparent limits over pretending to solve large models immediately.
- A native temporal checker adds correctness responsibility. The restricted fragment, reference engine, and independent oracle are release requirements, not optional polish.
- Familiar handler syntax may still be mistaken for deployable code. Documentation and CLI terminology must consistently say model and check, not run or deploy.
- Product-specific semantics require ongoing maintenance and can lag Cloudflare behavior. Explicit profile versions reduce, but do not remove, this burden.
- Restricting temporal composition is less expressive than TLA+. Clear rejection is preferable to an incorrect general-looking implementation.
- Retained request outcomes and pure state views need careful specification. They cannot stand in for arbitrary historical event properties.
- Initial KV/bucket/workflow rejection delays some attractive examples; implementing misleading approximations would be worse.

## Rationale and alternatives

### Proposed design

A Rust-native explicit-state checker with a limited temporal fragment gives direct access to Cloudflare-specific transitions, source-level counterexamples, and deterministic replay. Finite graph exploration is simple enough to audit before adding optimizations. Specialized temporal patterns cover safety, progress, response, recurrence, stabilization, and persistence without first building a general automata compiler.

### Rust frontend with TLC backend

This was an earlier candidate and remains a credible alternative. TLC provides mature invariant and temporal checking, fairness, and useful prior art. FML could lower its IR to TLA+ and reconstruct source traces.

Tradeoffs are a JVM/TLC runtime distribution, generated-model debugging, source-trace round-tripping, and preserving FML operation boundaries across two semantics. The proposal favors a native tool for the initial restricted fragment, but does not claim TLC is technically inferior. If independent validation exposes unacceptable risk or cost in native temporal checking, revisit this decision before release rather than dropping temporal support. The IR should remain capable of a future TLC backend, but v0 must not implement two production backends.

### Simpler trace simulator or property-based scheduler

Useful for fast bug finding and implementation testing, but sampling executions cannot establish absence of violations and finite traces alone cannot decide liveness. It can be a future mode labeled simulation; it does not satisfy the requested first-release contract.

### Safety-only checker

Substantially simpler, but does not meet the requirement for temporal operations. It is an internal vertical milestone, not the proposed delivered product.

### Full LTL/automata or SMT backend immediately

General LTL translation to a Büchi automaton and product emptiness checking would improve compositionality. SMT-backed bounded model checking could handle some data domains better. Both expand scope, dependencies, and proof obligations before primitive semantics are established. Bounded checks also require the same honest distinction between no bounded counterexample and an unbounded temporal result.

### TypeScript-embedded DSL

Familiar, but too easily confused with the implementation and host-language execution. It also makes it harder to constrain effects and retain a small formal core. Keep the agreed Riot-shaped standalone syntax.

### Generic stores and actors

A universal `store` erases distinctions between KV consistency, R2 object operations, and D1 relational constraints. A universal actor mailbox erases Worker lifecycle, Queue delivery, and DO identity/interleaving distinctions. Shared implementation helpers are fine; collapsing public semantics is not.

### C++

C++ can implement all proposed algorithms and may integrate with some existing tools. No concrete integration or performance requirement currently makes it cleaner than Rust. Use Rust unless future measurements identify a specific reason to change, not speculative speed arguments.

### Do nothing

Continue writing architecture diagrams and selected tests, or write raw TLA+ models manually. This avoids maintaining a language/checker but leaves Cloudflare semantics and developer-facing diagnostics to each project.

## Prior art and semantic references

- [Riot language](https://github.com/leostera/riot-lang/tree/e8127a13f25853c68cb373f5b7b3d4820d467924): borrow algebraic types, record/constructor forms, `let`, and braced `match`. Its process `send`/`link`/`monitor` do not automatically translate to Cloudflare actor lifecycle.
- [TLA+](https://lamport.azurewebsites.net/tla/tla.html) and [TLC](https://github.com/tlaplus/tlaplus): distinguish behaviors, safety, liveness, fairness, finite instances, and counterexample traces. FML adopts the distinctions without claiming language compatibility.
- [SPIN](https://spinroot.com/spin/whatispin.html): finite-state concurrency checking, temporal verification, and counterexample-guided debugging are central rather than incidental features.
- [Ecto schema](https://hexdocs.pm/ecto/Ecto.Schema.html): explicit typed schema declarations are a better fit than opaque SQL strings; FML tables represent database constraints as well as row shapes, not a full ORM.
- [Durable Object rules](https://developers.cloudflare.com/durable-objects/best-practices/rules-of-durable-objects/) and [SQLite storage API](https://developers.cloudflare.com/durable-objects/api/sqlite-storage-api/): single-threaded execution does not imply atomic handlers across external asynchronous operations; storage gates and commit boundaries matter.
- [Queues delivery guarantees](https://developers.cloudflare.com/queues/reference/delivery-guarantees/) and [batching/retries](https://developers.cloudflare.com/queues/configuration/batching-retries/): distinguish delivery, application effects, acknowledgment, retry, and exhaustion.
- [Workers KV consistency](https://developers.cloudflare.com/kv/concepts/how-kv-works/): cached absence and stale values matter; do not rely on same-location read-after-write visibility as a universal guarantee.
- [R2 consistency](https://developers.cloudflare.com/r2/reference/consistency/): direct bucket operations differ from cached custom-domain reads; the cache is a separate modeled component.
- [D1 read replication](https://developers.cloudflare.com/d1/best-practices/read-replication/): sessions/bookmarks change read semantics; primary-only v0 is an explicit scope choice, not a statement that every D1 read is globally current.

Cloudflare references were consulted during drafting. They inform the abstractions, but FML semantic profiles are independently specified models, not official Cloudflare guarantees. Each implemented primitive needs a maintained source/assumption ledger and litmus tests.

## Unresolved questions

### Before acceptance

- Confirm the first-release boundary: Worker + D1 first, with Queue + DO required before v0 release, and KV/bucket/workflow as subsequent vertical extensions.
- Confirm that the specified temporal patterns are sufficient initially, with arbitrary composition rejected rather than silently simplified.
- The native-checker decision is selected. Continue reviewing its validation coverage: the initial implementation uses an independent recurrent-edge-subset oracle on tiny graphs and fixed-point evaluation for replay.
- Review the queue duplicate/attempt abstraction and profile omissions before treating its traces as useful design evidence.

### During implementation

- Fix the complete grammar and diagnostic precedence without changing the examples' intended semantics.
- Choose default search budgets and trace-size safeguards using measured fixtures; no performance target is asserted yet.
- Specify the exact operation/result enums and queue transition table, including acknowledgment and retry accounting.
- Select the independent temporal oracle and keep its corpus reproducible in CI.
- Choose state storage/interning representations after implementing a simple exact reference engine.

### Out of scope

- How production code will be related to, tested against, or generated from a model.
- Deployment resource provisioning, D1 migration generation, and rolling-version compatibility checks.
- General observation/action predicates, arbitrary temporal formulas, and richer fairness.
- Full KV, R2, Workflow, alarm, timeout, and uncertain-commit semantic profiles.

## Future possibilities

Non-binding extensions include a TLC or symbolic backend, full LTL through automata, conservative symmetry/POR, model-to-code conformance testing, generated architecture diagrams, and interactive trace inspection. KV and R2 should demonstrate their differing read behavior through the same source-to-counterexample pipeline, not a cosmetic new keyword. Workflows should add durable step/retry semantics rather than a synonym for a Worker. None of these extensions justify weakening the first release's end-to-end checking and reporting contract.
