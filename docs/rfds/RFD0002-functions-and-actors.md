# RFD0002 — Functions and actors as the modeling core

**Status:** implemented generic core; stabilization and independent validation ongoing. This document is the current contract, not a promise of production conformance.

## Decisions

- One computational abstraction: an actor is a participant with a typed protocol, finite identity, and optional owned state. It may represent an algorithm, event loop, service, thread, or computer.
- One language and one execution contract. There is no `semantics` setting, legacy frontend, synchronous-call mode, or version-selectable interpreter.
- One claim declaration: `property`, with explicit `always`, a supported temporal form, or top-level `reachable`. There is no implicit interpretation of bare predicates.
- No compatibility burden for the initial experiments. Old syntax, Worker/D1 primitives, owner capabilities, suspended call frames, `invariant`, and `cover` declarations have been removed. Their history remains in git, not executable paths.
- Version trace artifacts when their representation or meaning changes; accept only the current format.
- Stable Rust for the application, tests, and normal CI. Nightly is optional tooling for coverage/sanitizer-instrumented fuzzing, not a runtime requirement.

## Motivation

Systems modeling should expose state transitions and interactions without claiming that their deployment technologies are equivalent. Pure functions express local decisions; actors delimit identity and owned state; messages express protocols. Replies, correlation, commit boundaries, and progress assumptions should be visible rather than hidden in implicit RPC.

The core is deliberately small and fault-free. A generic atomic turn is useful for reasoning but is not a distributed transaction, a database contract, or an implementation conformance proof.

## Language

### Data and functions

```fml
type AccountId = Alice | Bob
type RequestId = First | Second
type Request = Increment(Address<Client>, RequestId)
type Reply = Counted(RequestId, Int)
type ClientState = Waiting | Observed

let increment = (value: Int): Int { value + 1 }
```

Closed variants and records describe data. Built-ins are `Bool`, `Int`, `String`, `unit`, `Option<T>`, `Result<T, E>`, and `Address<ActorName>`. Transparent named aliases are supported; user-defined generics and recursive data are not.

Functions use typed parameters and an explicit return type (omitting the type means `unit`). The final expression or exhaustive tail match is the result. Blocks contain `let` bindings, expression statements, and `match`. Branch-local bindings do not leak. Shadowing globals/locals, recursive local calls, non-exhaustive matches, nested constructor patterns, and record-destructuring patterns are rejected. Match a payload in a second match or bind a record and inspect fields instead.

Result bindings must be matched immediately with explicit `Ok` and `Err` cases; wildcard disposal is rejected. Pure helpers, send helpers, and specification inspection are separated by conservative transitive effects. Send helpers can be called only as direct statements or bindings, not hidden inside arguments, records, constructors, or predicates.

Finite collection literals and data quantifiers are specification expressions. In `forall (x in T)`, a declared type name denotes its finite domain even if T also names a constructor; elsewhere constructor syntax retains its data meaning.

### Actors

```fml
actor Counter(id: AccountId) {
  init(id: AccountId): Int { 0 }
  handle_message(state: Int, message: Request): Int {
    match message {
      | Increment(reply_to, request_id) -> {
          let next = increment(state)
          send(reply_to, Counted(request_id, next))
          next
        }
    }
  }
}
actor Client {
  init(): ClientState { Waiting }
  handle_message(state: ClientState, message: Reply): ClientState {
    match message { | Counted(_, _) -> Observed }
  }
}
```

A singleton omits the key and its declaration name is its address. Keyed identities are eagerly instantiated from a finite named data type; `Counter.at(Alice)` has type `Address<Counter>`. All addresses exist before pure initializers run. Initialization is identity-only: keyed `init` takes the key (its parameter name need not match the declaration), singleton `init` takes no arguments.

A stateful handler takes `(state: State, message: Message)` and returns `State`. The state argument is a snapshot value, not a live owner capability. A stateless actor omits `init`, takes only the message, and returns `unit`. Each actor has exactly one `handle_message`; protocol variants are matched within it.

Addresses can travel through data and messages. They grant routing, not access to owned state. Read-only `Actor.state` and `Actor.at(key).state` are available in specifications only. Handlers and initializers cannot inspect state/observations through helpers either.

### Experiments

```fml
property "reply follows commit" {
  always (Client.state == Observed implies Counter.at(Alice).state == 1)
}
property "reply is possible" { reachable (Client.state == Observed) }
property "submitted work gets a reply" {
  forall (i in inputs(Counter)) { i.submitted leads_to Client.state == Observed }
}
check OneIncrement {
  domain Int = 0..1
  mailbox_bound = 2
  inputs { once send(Counter.at(Alice), Increment(Client, First)) }
  fairness { weak runtime.progress }
}
```

A check selects finite pools, bounds, external input slots, and optional fairness. The CLI selects a named check when more than one exists. It is not a program entry function. Each `once send` is optional and may be submitted at most once. There is no implicit startup event or dynamic spawn.

`Int` and `String` data require nonempty literal pools, at most 1025 entries. Integer ranges are inclusive; signed values can be given in literal lists. Domains are data, not computations or initializer calls. Each stored, sent, bound, helper-argument, and returned value is validated against the pools; arithmetic uses checked integers. Intermediate arithmetic subexpressions are not separate model transitions or stored bindings.

## Execution contract

1. Construct all finite addresses, evaluate pure initializers, and create empty mailboxes and unsubmitted input slots.
2. Always permit an explicit stutter transition.
3. For each unsubmitted input slot, permit one submission transition, appending its message to the target FIFO. Submission does not execute the handler.
4. For each nonempty mailbox, permit one processing transition: remove its head, evaluate the entire callback against the old state, validate its next state and staged outbox, install the new state, and append outgoing messages in source order. Input/message completion becomes true at this commit.

Different addresses can be scheduled in any order. One callback is a single atomic turn; no target can process its message until a later transition. Self-sends append after dequeue and observe the newly committed state on their next turn. A callback cannot suspend or perform external I/O.

Any callback or enqueue cutoff aborts the modeled transition: no partial state commit, observation allocation, or partial outbox publication is exposed. The explorer currently stops conservatively if any generated successor exceeds a bound; it does not treat capacity as a blocked send or silently explore a pruned graph.

FIFO is per target address, not global. Delivery/processing is fault-free: loss, duplication, crash, restart, timeout, retry, and durability are absent. Weak fairness affects scheduling, not whether a fault occurs.

### Fairness

`fairness { weak runtime.progress }` applies to each continuously enabled mailbox processing action. Action identity is based on its typed address, not a growing message sequence number. FIFO ensures a queued head cannot be overtaken. Optional external submissions remain unfair.

Safety and reachability use the full graph. Liveness uses fair infinite paths. Fairness checks use enabledness in the original graph, not only edges retained while searching a property-restricted subgraph. Fair self-edges and distinct action labels must be preserved even when data states coincide.

## Properties

| Formula | Obligation | Evidence/result |
| --- | --- | --- |
| `always P` | Safety | Finite bad prefix or verified in scope |
| `reachable P` | Existential finite reachability | `REACHED` witness; `UNREACHABLE` only on a closed graph |
| `eventually P` | Universal eventuality | Fair-lasso counterexample |
| `P leads_to Q` | Response | Fair-lasso counterexample |
| `P until Q` | Strong until | Finite bad prefix or fair lasso |
| `always eventually P` | Recurrence | Fair lasso |
| `eventually always P` | Stabilization | Fair lasso |
| `always (P implies always Q)` | Persistence | Finite bad prefix |

P and Q must be state predicates. Temporal conjunction and stable universal quantification over type domains or `inputs`/`messages` are allowed. Temporal disjunction, negation, arbitrary nesting, `next`, and strong fairness are rejected. A bare `property { P }` is rejected.

Only a whole property body `reachable P` is supported. P may contain data quantifiers and pure Boolean operations, but no temporal or reachability operators. Reject `always (reachable P)`, `reachable (eventually P)`, and mixtures of reachability and temporal clauses. `exists` is a data quantifier, not an execution quantifier.

A whole `always P` with a state predicate is classified as safety and checked at initialization and each newly discovered state, before graph closure or later exploration cutoffs. `always eventually P` is not safety. Conjunctions/quantified temporal clauses retain the temporal analysis path, which requires graph closure.

Reached witnesses remain valid during incomplete exploration. Missing witnesses do not establish unreachability unless exploration is complete. An unreachable query is informational, not a failed requirement or nonzero exit. Data-domain/evaluation/graph cutoffs are inconclusive, never evidence of verification. Internal report/trace kinds identify the obligation algorithm; they are not additional source declaration keywords.

### Precedence

From low to high: `leads_to`/`until`, `implies`, `||`, `&&`, equality (`==`/`!=`), ordered comparisons (`<`/`>`/`<=`/`>=`), `+`/`-`, prefix operators, and field/call postfix operations. Prefix `always`, `eventually`, and `reachable` bind like other unary operators. Use parentheses around compound predicates: `always (x == y)`, not `always x == y`.

## Stable observations

`inputs(A)` ranges over A's external input slots across all keys, including before submission. Fields: `payload: Message`, `target: Address<A>`, `submitted: Bool`, `processed: Bool`. The two flags are monotone. Completion means only that slot's own callback committed—not that a generated reply or follow-up finished.

`messages(A)` requires `message_bound = N` and ranges over N stable potential slots from initialization. Each enqueue allocates a fresh slot for the target actor declaration, across all keys, with fields:

- `sent`, `processed`, `external`: Boolean flags;
- `payload: Option<Message>` and `target: Option<Address<A>>`;
- before allocation: false flags and `None` data.

Identical payloads get distinct slots; slots never recycle. Temporal bindings therefore retain identity, including for generated messages absent at initialization. Exhausting lifetime slots is inconclusive. Spare unused slots may have unreached antecedents; this is reported as a count, not as if the entire property ranged over an empty collection.

`mailbox_bound = N` is required and bounds pending envelopes **per address**. `message_bound = N` is optional and bounds lifetime observed sends **per actor declaration**. Both permit 1..4096. A finite-state self-sending cycle can be checked without lifetime history; adding finite history cannot establish unbounded-message progress by recycling slots.

## Implementation and evidence

- `syntax.rs`: Logos lexer with tokens/source spans, recursive-descent declarations/statements, and Pratt expression parsing. No parser-generator grammar or ad hoc regex parser.
- `model.rs`: names/types; `functions.rs`: acyclic dependency and effect inference.
- `claims.rs`: property classification; `semantics.rs`: values/domains and one shared statement evaluator; `messaging.rs`: initialization and atomic transitions; `observations.rs`: stable observation identities.
- `checker.rs`: exact-state BFS, early safety/reachability, conservative cutoffs.
- `graph.rs` / `temporal.rs`: reachability, fair recurrent SCCs, restricted temporal checking.
- `trace.rs`: current-format artifacts; replay re-executes actions, compares every state and provenance, validates source/check/bounds, loop closure and original enabledness. A fixed-point interpreter independently validates the failed formula.
- `main.rs`: clap-derived CLI; `diagnostics.rs`: ANSI-safe text reports separate from JSON.

Current trace format is **6**. Older formats, unknown fields, altered actions, snapshots, identities, bounds, or source hashes are rejected. The format number is not a semantics switch.

Normal validation uses stable Rust: formatting, strict Clippy, source/CLI tests, replay tests, and independent finite oracles. Temporal validation exhausts all two-state topology/fairness/predicate combinations for seven patterns and samples three-state graphs with shared action IDs. A mailbox oracle compares edge labels, queue/input identities and cutoffs. A separate state-machine oracle exhausts all 729 deterministic three-state/two-message transition tables and compares every reachable transition. These are bounded independent checks, not a proof.

## Host safety and limits

These guards produce invalid-source errors for unsupported static structure, or runtime inconclusive cutoffs; they do not claim production bounds:

- CLI source size: 1 MB; trace input: 16 MB; replay: at most 100,000 actions.
- Parser expression/tree nesting: 128; local call/data-type depth: 64.
- Expanded local function cost: 10,000 nodes.
- Evaluation nesting: 128; work: 100,000 entries per outer evaluation, reset after errors.
- Individual values: 4096 nodes / 64 levels; enumerated domain products and total addresses: 4096; domain enumeration nesting: 24.
- Temporal expansion: 4096 clauses and 10,000 expansion steps, including paths ending in empty domains.
- Default graph budgets: 100,000 states, depth 1000, 30 seconds. Timeouts are cooperative, not hard OS deadlines.

No model can execute host network APIs or arbitrary Rust code. Treat modeled data as potentially sensitive; traces contain values and addresses. Escape terminal controls and do not collect telemetry.

## Scope and next work

The generic core and unified property surface are implemented. Remaining **stabilization acceptance** is continued coverage-instrumented fuzzing, seeded replay corruption campaigns, regression minimization, and independent review of the implementation against this contract. An optional nightly fuzz workflow is separate from stable development CI; see the [checklist](RFD0002-implementation-checklist.md) for actual local validation status.

The original proposal coupled generic-core acceptance to product adapters. That is no longer the merge boundary: no Worker/DO/Queue/D1 adapter is implemented or implied, and there is no profile registry. Resource models may later be explicit libraries/protocols, with truthful scheduling, storage, and failure rules. A model must split publication and state commit when its real system cannot justify atomicity.

Next language design work, after stabilization: imports/namespaces, reusable actor definitions, finite static instances, and explicit initialization configuration. Those need a separate design, especially instance/address typing and imported-source replay identity. Do not add dynamic spawning, automatic RPC, suspension, or hidden compatibility modes as part of this stabilization pass.

Out of scope: production code generation/conformance proofs, supervision, timers, crashes/restarts, automatic retry, shared-memory consistency models, unbounded checking, and backend equivalence claims. Partial-order/symmetry reductions are potential future engine work and require their own soundness validation.
