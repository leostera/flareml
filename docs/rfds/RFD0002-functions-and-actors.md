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
type RequestId = First | Second
type Request = Increment(Actor<Client>, RequestId)
type Reply = Counted(RequestId, Int)
type ClientState = Waiting | Observed

let increment = (value: Int): Int { value + 1 }
```

Closed variants and records describe data. Built-ins are `Bool`, `Int`, `String`, `unit`, `Option<T>`, `Result<T, E>`, and `Actor<ActorName>`. The latter is a typed reference to a declared actor, not an owned-state capability. Transparent named aliases are supported; user-defined generics and recursive data are not.

Functions use typed parameters and an explicit return type (omitting the type means `unit`). The final unterminated expression or exhaustive tail match is the result. Blocks contain `let` bindings, expression statements, and `match`. Every `let` binding and every non-tail statement requires `;`, including a non-tail `match`. A terminated final expression discards its value and makes the block return `unit`; an empty block also returns `unit`. Match arms are separated by `|`, not semicolons; use a braced block for multiple statements within an arm. Whitespace/newlines alone do not separate statements. Branch-local bindings do not leak. Shadowing globals/locals, recursive local calls, non-exhaustive matches, nested constructor patterns, and record-destructuring patterns are rejected. Match a payload in a second match or bind a record and inspect fields instead.

Result bindings must be matched immediately with explicit `Ok` and `Err` cases; wildcard disposal is rejected. Pure helpers, send/choice/spawn helpers, and specification inspection are separated by conservative transitive effects. Effectful helpers can be called only as direct statements or bindings, not hidden inside arguments, records, constructors, or predicates. [RFD0003](RFD0003-nondeterministic-choice-and-faulty-links.md) adds `let value = choose([candidate, ...]);`: nonempty literal lists of compatible pure expressions, only as a whole local binding initializer during handler execution. Choice is excluded from setup, initialization, specifications, and external input declarations.

Top-level `let` declares functions; function/handler blocks support local value bindings. Property bodies are expressions, not statement blocks, so they do not directly accept `let`. A pure or specification-only helper may use local bindings and be called from a property.

Finite collection literals and data quantifiers are specification expressions. In `forall (x in T)`, a declared type name denotes its finite domain even if T also names a constructor; elsewhere constructor syntax retains its data meaning.

### Actors

```fml
actor Counter {
  init(): Int { 0 }
  handle_message(state: Int, message: Request): Int {
    match message {
      | Increment(reply_to, request_id) -> {
          let next = increment(state);
          send(reply_to, Counted(request_id, next));
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

Every actor declaration defines a type and creates **no instances**. `spawn(Counter)` creates a fresh `Actor<Counter>` reference in setup or a handler. There are no implicit singleton/keyed populations, `.at(...)` addresses, or `spawnable` modifier. Definition names are not routing references.

Each selected check requires `spawn_bound Type = N` in 0..4096 for every actor definition, including unused ones. Initial and later creation consume the same monotone lifetime slots, independent of `Int` domains, never reused. `spawn(Type, args...)` supplies typed, pure arguments to `init`; stateless definitions take none. Initializers cannot send, choose, spawn, or inspect observations. Allocation must be a direct statement or binding, including calls to allocating helpers, never nested in pure arguments or predicates. There is no implicit self binding or startup message: the creator can explicitly send the new reference to itself. [RFD0004](RFD0004-bounded-spawn.md) specifies setup, initialization, and allocation.

A stateful handler takes `(state: State, message: Message)` and returns `State`. The state argument is a snapshot value, not a live owner capability. A stateless actor omits `init`, takes only the message, and returns `unit`. Each actor has exactly one `handle_message`; protocol variants are matched within it.

Addresses can travel through data and messages. They grant routing, not access to owned state. Read-only state observations are available through `instances(Type)` in specifications only, not through routing references or definition names. Handlers and initializers cannot inspect state/observations through helpers either.

### Experiments

```fml
property "reply follows commit" {
  always ((exists (client in instances(Client)) { client.state == Some(Observed) }) implies
    (forall (counter in instances(Counter)) { counter.state == Some(1) }))
}
property "reply is possible" { reachable (exists (client in instances(Client)) { client.state == Some(Observed) }) }
property "submitted work gets a reply" {
  forall (i in inputs(Counter)) { i.submitted leads_to (exists (client in instances(Client)) { client.state == Some(Observed) }) }
}
check OneIncrement {
  domain Int = 0..1
  mailbox_bound = 2
  spawn_bound Counter = 1
  spawn_bound Client = 1
  main {
    let counter = spawn(Counter);
    let client = spawn(Client);
    inputs { once send(counter, Increment(client, First)) }
  }
  fairness { weak runtime.progress }
}
```

A check selects finite pools, bounds, a required `main` setup block, and optional fairness. The CLI selects a named check when more than one exists. Only its `main` runs, once and deterministically before exploration. `main {}` leaves an empty population. Setup may spawn and send, including through helpers, but cannot choose or inspect observations. Actors do not process until setup completes. Setup bindings stay local; pass references explicitly.

Within setup, `inputs { once send(reference, payload) }` captures optional external slots using pure expressions in the local environment. Each slot may be submitted at most once; registration does not enqueue it. In contrast, `send` in setup guarantees initial enqueueing, but not processing without the relevant fairness assumptions. An intermediate inputs block requires `;`, like other statements. No setup-local binding becomes a global property variable. There is no implicit startup event.

`Int` and `String` data require nonempty literal pools, at most 1025 entries. Integer ranges are inclusive; signed values can be given in literal lists. Domains are data, not computations or initializer calls. Each stored, sent, bound, helper-argument, and returned value is validated against the pools; arithmetic uses checked integers. Intermediate arithmetic subexpressions are not separate model transitions or stored bindings.

## Execution contract

1. Start with no instances. Execute the selected check's deterministic setup through the shared interpreter. Publish all its allocations and source-ordered sends, then expose captured unsubmitted input slots as the initial state. A setup cutoff produces no partial initial state or witness.
2. Always permit an explicit stutter transition.
3. For each unsubmitted input slot, permit one submission transition, appending its message to the target FIFO. Submission does not execute the handler.
4. For each nonempty mailbox, explore the callback against the old state and head message. Every complete local choice execution permits one processing transition: remove the head, validate the next state, branch-local allocation reservations, and staged outbox; install new instance state/mailboxes and sender state, and append outgoing messages in source order. Input/message completion becomes true at this commit. Choice alternatives do not create intermediate scheduling boundaries.

Different addresses can be scheduled in any order. One callback is a single atomic turn; no target can process its message until a later transition. Self-sends append after dequeue and observe the newly committed state on their next turn. A callback cannot suspend or perform external I/O.

Any callback or enqueue cutoff aborts the modeled transition: no partial state commit, actor allocation, observation allocation, or partial outbox publication is exposed. New references can be retained and sent to within a turn, but no created actor runs or becomes visible to another participant before commit. The explorer currently stops conservatively if any generated successor exceeds a bound; it does not treat capacity as a blocked send or silently explore a pruned graph.

FIFO is per target address, not global. The engine does not inject loss, duplication, crashes, restarts, timeouts, retries, or durability. A protocol actor may explicitly choose to drop or duplicate a packet before onward delivery. Weak fairness affects scheduling, not the selection of a choice alternative.

### Fairness

`fairness { weak runtime.progress }` applies to each continuously enabled mailbox processing action. Action identity is based on its typed address, not a growing message sequence number or choice transcript. Any completed choice outcome services the same mailbox action; alternatives are not individually fair. FIFO ensures a queued head cannot be overtaken. Optional external submissions remain unfair.

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

P and Q must be state predicates. Temporal conjunction and stable universal quantification over type domains or `inputs`/`messages`/`instances` are allowed. Temporal disjunction, negation, arbitrary nesting, `next`, and strong fairness are rejected. A bare `property { P }` is rejected.

Only a whole property body `reachable P` is supported. P may contain data quantifiers and pure Boolean operations, but no temporal or reachability operators. Reject `always (reachable P)`, `reachable (eventually P)`, and mixtures of reachability and temporal clauses. `exists` is a data quantifier, not an execution quantifier.

A whole `always P` with a state predicate is classified as safety and checked at initialization and each newly discovered state, before graph closure or later exploration cutoffs. `always eventually P` is not safety. Conjunctions/quantified temporal clauses retain the temporal analysis path, which requires graph closure.

Reached witnesses remain valid during incomplete exploration. Missing witnesses do not establish unreachability unless exploration is complete. An unreachable query is informational, not a failed requirement or nonzero exit. Data-domain/evaluation/graph cutoffs are inconclusive, never evidence of verification. Internal report/trace kinds identify the obligation algorithm; they are not additional source declaration keywords.

### Precedence

From low to high: `leads_to`/`until`, `implies`, `||`, `&&`, equality (`==`/`!=`), ordered comparisons (`<`/`>`/`<=`/`>=`), `+`/`-`, prefix operators, and field/call postfix operations. Prefix `always`, `eventually`, and `reachable` bind like other unary operators. Use parentheses around compound predicates: `always (x == y)`, not `always x == y`.

## Stable observations

`instances(A)` is specification-only for every actor definition and ranges over all potential creation slots from initialization. Fields are `created: Bool`, `reference: Option<Actor<A>>`, and, for stateful definitions, `state: Option<StateType>`. Before creation these are false/None; after creation the reference remains stable and state reflects later commits. Stateless instances have no state field. Enumeration of data domains containing actor routing references is unsupported rather than fabricating unborn references; use instance slots.

`inputs(A)` ranges over external slots captured for A's instances during setup, including before submission; the view may be empty. Fields: `payload: Message`, `target: Actor<A>`, `submitted: Bool`, `processed: Bool`. The two flags are monotone. Completion means only that slot's own callback committed—not that a generated reply or follow-up finished.

`messages(A)` requires `message_bound = N` and ranges over N stable potential slots from initialization. Each enqueue, including a setup send, allocates a fresh slot for the target actor definition across all instances, with fields:

- `sent`, `processed`, `external`: Boolean flags;
- `payload: Option<Message>` and `target: Option<Actor<A>>`;
- before allocation: false flags and `None` data.

Identical payloads get distinct slots; slots never recycle. Temporal bindings therefore retain identity, including for generated messages absent at initialization. Exhausting lifetime slots is inconclusive. Spare unused slots may have unreached antecedents; this is reported as a count, not as if the entire property ranged over an empty collection.

`mailbox_bound = N` is required and bounds pending envelopes **per address**. `message_bound = N` is optional and bounds lifetime observed sends **per actor declaration**. Both permit 1..4096. A finite-state self-sending cycle can be checked without lifetime history; adding finite history cannot establish unbounded-message progress by recycling slots.

## Implementation and evidence

- `syntax.rs`: Logos lexer with tokens/source spans, recursive-descent declarations/statements, and Pratt expression parsing. No parser-generator grammar or ad hoc regex parser.
- `model.rs`: names/types; `functions.rs`: acyclic dependency and effect inference.
- `claims.rs`: property classification; `semantics.rs`: values/domains and one shared statement evaluator; `messaging.rs`: initialization and atomic transitions; `observations.rs`: stable observation identities.
- `choices.rs`: bounded prefix enumeration through the shared local interpreter; exact encounter/call-site/candidate evidence and constrained replay.
- `spawning.rs`: branch-local fresh allocation and stable instance observations; registry/mailbox publication is part of the atomic messaging commit.
- `checker.rs`: exact-state BFS, early safety/reachability, conservative cutoffs.
- `graph.rs` / `temporal.rs`: reachability, fair recurrent SCCs, restricted temporal checking.
- `trace.rs`: current-format artifacts; replay re-executes actions, compares every state and provenance, validates source/check/bounds, loop closure and original enabledness. A fixed-point interpreter independently validates the failed formula.
- `main.rs`: clap-derived CLI; `diagnostics.rs`: ANSI-safe text reports separate from JSON.
- `run_artifacts.rs`: CLI run bundles under `.fml/runs/<unique-id>/` (override parent with `--artifacts-dir`): exact source, requested configuration/source hash/tool version, report, and every available witness. `report.json` is the final completion marker. Verified properties have no fabricated proof trace; incomplete runs retain only genuine evidence. Source/trace replay is authoritative for witnesses; a saved report is not a proof certificate. Unreadable source and storage errors can prevent bundle completion.

Current trace format is **8**. Replay re-executes deterministic setup and compares the entire initial snapshot, including registry, queues, captured input payloads/targets/source spans, and observations. Setup is not a scheduler action or a fairness obligation. Each action has a choice transcript and allocation records, empty when unused. Replay constrains choices, reconstructs fresh allocations/initialization, checks creation bounds and registry/mailboxes, and compares the complete action and state. Fair enabledness is derived from the original state's nonempty mailboxes, independently of the chosen outcomes. Older formats, unknown fields, altered actions, snapshots, identities, bounds, or source hashes are rejected. The format number is not a semantics switch.

Normal validation uses stable Rust: formatting, strict Clippy, source/CLI tests, replay tests, and independent finite oracles. Temporal validation exhausts all two-state topology/fairness/predicate combinations for seven patterns and samples three-state graphs with shared action IDs. A mailbox oracle compares edge labels, queue/input identities and cutoffs. A separate state-machine oracle exhausts all 729 deterministic three-state/two-message transition tables and compares every reachable transition. These are bounded independent checks, not a proof.

## Host safety and limits

These guards produce invalid-source errors for unsupported static structure, or runtime inconclusive cutoffs; they do not claim production bounds:

- CLI source size: 1 MB; trace input: 16 MB; replay: at most 100,000 actions.
- Parser expression/tree nesting: 128; local call/data-type depth: 64.
- Expanded local function cost: 10,000 nodes.
- Evaluation nesting: 128; work: 100,000 entries per outer evaluation, reset after errors. All prefix executions of one turn share that work budget.
- Choice: 128 encounters per turn; 4096 candidates per encounter and 4096 prefix executions per turn (including incomplete prefixes). Guards can therefore fire before 4096 completed outcomes. Expansion polls the check deadline; exhausting any guard is inconclusive.
- Individual values: 4096 nodes / 64 levels; enumerated domain products and total addresses: 4096; domain enumeration nesting: 24.
- Temporal expansion: 4096 clauses and 10,000 expansion steps, including paths ending in empty domains.
- Default graph budgets: 100,000 states, depth 1000, 30 seconds. Timeouts are cooperative, not hard OS deadlines.

No model can execute host network APIs or arbitrary Rust code. Treat modeled data as potentially sensitive; traces contain values and addresses. Escape terminal controls and do not collect telemetry.

## Scope and next work

The generic core and unified property surface are implemented. Remaining **stabilization acceptance** is continued coverage-instrumented fuzzing, seeded replay corruption campaigns, regression minimization, and independent review of the implementation against this contract. An optional nightly fuzz workflow is separate from stable development CI; see the [checklist](RFD0002-implementation-checklist.md) for actual local validation status.

The original proposal coupled generic-core acceptance to product adapters. That is no longer the merge boundary: no Worker/DO/Queue/D1 adapter is implemented or implied, and there is no profile registry. Resource models may later be explicit libraries/protocols, with truthful scheduling, storage, and failure rules. A model must split publication and state commit when its real system cannot justify atomicity.

[RFD0003: choice and faulty links](RFD0003-nondeterministic-choice-and-faulty-links.md) and [RFD0004: bounded spawn](RFD0004-bounded-spawn.md) are implemented and reflected in this contract. [RFD0005: suspension and reentrancy](RFD0005-suspension-and-reentrancy.md) remains an unimplemented sketch. Refine, implement, and validate each milestone separately without compatibility profiles.

Constants, imports/namespaces, and generic reusable definitions remain separate design work. Imports must resolve replay source identity. Explicit setup and dynamic creation share `spawn` and the same lifetime bounds. No automatic RPC is implied.

Out of scope: production code generation/conformance proofs, supervision, timers, crashes/restarts, automatic retry, shared-memory consistency models, unbounded checking, and backend equivalence claims. Partial-order/symmetry reductions are potential future engine work and require their own soundness validation.
