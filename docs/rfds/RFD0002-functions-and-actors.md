# RFD0002 - Functions and actors as the modeling core

- Feature Name: `functions-and-actors`
- Status: Draft
- Mode: Proposal
- Author: leostera, with AI assistance
- Start Date: 2026-09-26
- Updated: 2026-09-26
- Implementation: [actor-generalization spike](../spikes/actor-generalization.md) on `spike/actor-generalization` (`actors-v1` vertical slice; proposal not fully implemented)

## Summary

Make typed functions and actors the general modeling core of FML. Functions describe computations and explicit effects; actor declarations bind those functions to identities, protocols, and optional owned state. A stateless actor has independent invocations; a stateful actor owns a value per modeled identity. Neither term implies a Cloudflare Worker, Durable Object, persistence, or an atomic handler. Product-specific semantics remain separate, versioned profiles/adapters that must specify their operations rather than being inferred from the generic actor keyword. Preserve the native checker, finite-scope and temporal-property contracts from [RFD0001](RFD0001-initial-language-and-model-checker.md). Extend the working spike vertically: typed addresses, calls with suspended continuations, independently scheduled keyed instances, and replayable counterexamples. This RFD proposes the target contract; it does not claim those extensions are already implemented.

## Motivation

RFD0001 starts from a Cloudflare-native vocabulary and an initial Worker/D1 implementation. The [first checkpoint](../../README.md) checks real `.fml` source and catches a login bug and a D1 lost update. But declaring every handler inside `worker` or `durable` entangles the *behavior being modeled* with its *execution environment*. This makes reuse, ordinary pure functions, and non-Cloudflare systems awkward. It also suggests incorrectly that a named Worker is a long-lived actor, or that a Durable Object handler is atomic end-to-end.

The branch spike has working top-level functions, stateless actors, and single-instance stateful actors. It validates the direction: [`actor-stateless.fml`](../../examples/actor-stateless.fml) checks a reusable decision function without Cloudflare resources; [`actor-counter.fml`](../../examples/actor-counter.fml) finds a state invariant failure and replays it. At drafting the spike was **not** yet a general actor calculus: one stateful instance per name, no inter-actor calls, no address protocol, no restarts or persistence. The subsequent `actors-v1` slice adds finite keyed identities and direct fault-free calls, and first-class `Address<ActorName>` values, but still has no restart or persistence semantics. Its `worker`/`cf-core-v0` parser shim exists to retain old tests, not to determine the eventual public surface.

We need a design that makes the boundaries teachable and checkable: what runs locally, who owns state, how actors are addressed, what crosses a message boundary, when another invocation can run, and which properties have actually been established.

## Goals

- Separate function definition from actor binding, and allow reuse of pure computations in handlers and specifications under explicit effect rules.
- Define typed, finite actor identities and request/reply protocols; support both named stateless services and keyed stateful instances.
- Preserve direct source-to-checker execution, explicit scheduling/fairness, state-by-state invariants, liveness lassos, and replay.
- Keep actor state and input-slot/continuation identity in state hashing and trace evidence.
- Make backend mappings (Worker, Durable Object, Queue, D1) visible, versioned semantic choices without making `worker` a *core* language construct or collapsing distinct product contracts.
- Retain a safe, explicit migration path for RFD0001 examples and artifacts.

## Non-goals

- Compiling or deploying actor models, verifying separately written production implementations, or claiming runtime conformance.
- Inferring Cloudflare durability from `stateful`; inferring exactly-once delivery, a queue mailbox, or global ordering from `actor`.
- Arbitrary recursion, higher-order closures, unbounded identities/messages, general distributed transactions, or an unbounded proof engine in this phase.
- A generic `Store` that erases D1/KV/R2 behavior, or a general function-library facility for inventing unvalidated resource guarantees.
- Silent fallback when an address, effect, profile, capacity, or failure behavior is unsupported.

## Guide-level explanation

### Functions compute; actors own boundaries

An illustrative source file, **working now** on the spike branch:

```fml
type Eligibility = Eligible | Ineligible
type Request = Request { eligible: Eligibility }
type Reply = Allowed | Denied

let decision = (eligible: Eligibility): Reply {
  match eligible {
    | Eligible -> Allowed
    | Ineligible -> Denied
  }
}

let login = (request: Request): Reply {
  decision(request.eligible)
}

stateless actor API {
  handle_request = login
}

invariant "ineligible users cannot log in" {
  forall (r in requests(API.handle_request)) {
    r.response == Some(Allowed) implies r.input.eligible == Eligible
  }
}

property "accepted requests eventually return" {
  forall (r in requests(API.handle_request)) {
    r.accepted leads_to r.completed
  }
}

check Login {
  semantics = "actors-v0"
  inputs {
    once API.handle_request(Request { eligible: Eligible })
    once API.handle_request(Request { eligible: Ineligible })
  }
  fairness { weak runtime.progress }
}
```

The function bodies use ML-shaped types, `match`, and a trailing result expression. The actor binds an entrypoint to a function; it does not copy or magically execute the function in a new process. `requests(...)` observes finite input slots, including those not yet accepted. `once` creates one optional input each. Fairness applies to continuously enabled modeled progress actions, not to optional input arrival.

An illustrative **working single-instance** stateful model:

```fml
type Reply = Count(Int)

let increment = (owner: Actor<Int>, amount: Int): Reply {
  owner.set(owner.state + amount)
  Count(owner.state)
}

stateful actor Counter {
  state: Int = 0
  add = increment
}

invariant "counter never exceeds one" { Counter.state <= 1 }

check Concurrent {
  semantics = "actors-v0"
  domain Int = 0..2
  inputs { once Counter.add(1) once Counter.add(1) }
  fairness { weak runtime.progress }
}
```

`fml check examples/actor-counter.fml` reports the second call that makes `Counter.state == 2`. Here state is retained **in the model** between invocations; no claim of storage persistence or eviction behavior follows. `owner` is an owned capability, not an actor address to serialize or pass to another actor. The top-level `Counter.state` view is available to properties, not to other actors' handler code.

### The next vertical example: two actors

The following was **proposed syntax when this draft was written**. The branch now accepts the keyed address and call forms under the experimental `actors-v1` profile, without implying that the rest of this RFD is implemented. It illustrates why `call` cannot be treated as a local function invocation:

```fml
type AccountId = Alice | Bob
type Reply = Balance(Int)

let deposit = (owner: Actor<Int>, amount: Int): Reply {
  owner.set(owner.state + amount)
  Balance(owner.state)
}

stateful actor Account(id: AccountId) {
  state: Int = 0
  deposit = deposit
}

let send_deposit = (id: AccountId): Reply {
  call(Account.at(id).deposit, 1)
}

stateless actor API { transfer = send_deposit }

invariant "different accounts do not share state" {
  Account.at(Alice).state == 0 || Account.at(Bob).state == 0
}

check Deposits {
  semantics = "actors-v1"
  domain Int = 0..2
  inputs { once API.transfer(Alice) }
  fairness { weak runtime.progress }
}
```

The property is illustrative and intentionally weak; actual acceptance fixtures should assert concrete per-key outcomes and use a cover to avoid vacuity. `Account.at(id)` denotes a stable logical identity and `call` selects a typed handler and carries a serializable message, not an immediate nested Rust call. The version label `actors-v1` is now allocated to the branch's fault-free, finite request/reply slice with scheduler and replay tests; first-class keyed address values use `Address<ActorName>` and can cross a message boundary. The profile does not imply fault handling, durability, or a Cloudflare backend. Additional behavior requires new profile contracts and tests.

A useful counterexample should show input acceptance, call issue, target identity, target acceptance, state commit, reply, resumed caller, and the violated predicate with bound IDs. A failed `fml check` is a behavior of the declared finite model and assumptions, **not** proof of a deployed bug.

## Reference-level explanation

### Architecture and boundaries

Maintain three layers:

1. **General modeling language:** algebraic data types, pure functions, effectful functions with explicit capabilities, stateless/stateful actor bindings, initial states, input slots, properties and scopes. This layer defines no Cloudflare product name.
2. **Semantic profiles/adapters:** define available resource operations, typed capabilities, message/call failure modes, state persistence, consistency, queue delivery and scheduling rules. Product-specific tables (`d1`), `kv` and `bucket` may remain declarative syntax supplied by a Cloudflare-facing frontend or feature set; this RFD changes the **core actor syntax**, not the proposal for distinct storage semantics.
3. **Native checker and diagnostics:** the existing typed IR, explicit-state graph, fairness and temporal engines, witnesses and replay. Preserve the checker contract from RFD0001; don't replace it with an unchecked interpreter.

The Rust modules remain a small single-package codebase. `src/syntax.rs` parses functions and actors; `src/functions.rs` binds functions and infers conservative transitive effects; `src/model.rs` typechecks/lowers; `src/semantics.rs` evaluates and schedules; `src/graph.rs`, `src/temporal.rs`, `src/checker.rs`, and `src/trace.rs` check and validate results. The future separation is a semantic boundary first, not a requirement to publish many crates.

### Functions, types and capabilities

- A function has named typed parameters, declared result type, lexically scoped `let`/`match` and a trailing result expression on each reachable branch. Non-unit functions with missing return paths are invalid. A function body does not imply an invocation, a message, a transaction or a suspension.
- Local calls to *pure* functions evaluate deterministically inside the current local segment. Transitive effects must be included in checking: a helper that reads D1 or calls an actor does not become atomic merely because it has a function name. An effectful call is lowered to the same semantic boundaries it would have inline, with explicit caller continuation and call-site provenance.
- Specification inspectors may read modeled state without invoking a resource operation. They are pure from the checker's perspective, but may not be used as a shortcut inside executable handlers. No accidental ambient access to another actor's state.
- `Actor<State>` is an unforgeable, scoped owner capability supplied only to handlers of that actor. It may be passed to safe local helpers but may not be returned, stored in records/tables, sent as a message or captured across another actor boundary. This must hold transitively through aliases, variants, collections and function results; reject all unimplemented escaping forms. A separate typed `Address<ActorName>` may be stored or passed when its key domain is finite and serializable; the key type comes from the named actor's declaration. This experimental surface remains open to review.
- A stateless actor binds functions of `(message: Input) -> Reply`; each invocation gets fresh locals. A stateful actor binds `(owner: Actor<State>, message: Input) -> Reply`; state is owned by the identified instance. Other actors do not receive its owner capability.
- Only supported finite data types may cross a boundary; crossing cannot expose call stack frames, functions/closures or owner capabilities. The typechecker rejects unsupported calls rather than pretending arbitrary expressions serialize.

The spike has pure function calls, inferred effects, and now singleton/keyed `Actor<State>` owners in `actors-v1`, but does not yet enforce all of these rules in a proven general effect/capability system. Treat its implementation as evidence to refine, not proof that the intended discipline is sound.

### Identity, state and scheduling

- A stateless actor name identifies a service endpoint, **not** one retained process. Multiple accepted invocations have independent local frames and may overlap at allowed boundaries.
- For keyed stateful actors, a name and a key select an instance: the same `(actor, key)` addresses the same modeled state; different keys never implicitly share an owner capability or state. Initialize each key from the actor's declared initial expression, either eagerly in a finite check or lazily with equivalent property-visible state. The active key domain must be finite and stated in the check. A singleton stateful actor is the special case with one unit key.
- An invocation executes deterministic local operations until its next semantic boundary. A local `set` and following pure computation may belong to one transition if no intervening external effect exists. Calls and external operations split into issue and completion steps; the caller waits with a retained continuation while independently enabled invocations may run, including another invocation of the same stateful actor. **No entire-handler lock is inferred.** A read before suspension and a write after resumption can race.
- An owned-state write commits at its local transition boundary. The generic `actors-v0` state is retained in model state but makes no persistence/restart guarantee. A durability adapter must say which commits survive restart, what happens to volatile locals and outstanding operations, and how its gating/transactions alter interleaving. Do not label the generic stateful actor as a Durable Object in checker output.
- A typed actor call has (1) issue with caller continuation and target address, (2) target acceptance and execution, (3) reply publication, and (4) caller resume. A synchronous response is distinct from one-way delivery and Queue submission. The initial call profile should explicitly exclude transport failure and timeouts **and print that exclusion**; later profiles model known non-commit and uncertain outcome separately. Reply cannot be observed before the target's modeled commit. No implicit exactly-once semantics for queues or external side effects.
- Direct local recursion remains rejected in a first finite implementation. A graph of actors calling one another may be cyclic; do not falsely reject all such topologies as recursive local functions. Instead track pending calls, finite IDs and capacity. If expansion can exceed representational capacity, report `INCONCLUSIVE` with the boundary, never silently drop an invocation. Define and test deadlock on mutual waiting, including how weak fairness treats truly disabled actions.

### Properties, state space and results

Preserve RFD0001's safety and temporal semantics: check invariants at initialization and **every reachable boundary**, including while a caller waits and after an actor commits but before its reply; check fair infinite paths for liveness; require a finite explicit domain for keys/messages and distinct input slots. `requests(Actor.handler)` is the read-only view of the check's finite external invocations; a distinct future view is needed for dynamically created actor-to-actor calls. Do not silently mingle or discard either category in quantifiers. A keyed property such as `Account.at(id).state` observes a **single instantaneous state** without calling the actor and must quantify over a declared finite key domain.

State identity includes keyed actor values, active invocation and pending call identities, program counters, locals needed for future computation, and correlation between caller and callee. Trace and replay evidence include semantic-profile version, normalized source/check identity, enabled action labels, scope and fairness. Preserve distinct edges even when their data state is equal, and preserve history-sensitive temporal monitors. Capacity or depth/time cutoffs remain inconclusive; increasing a budget cannot transform an unexplored model into a verified one by dropping actions.

### Product mappings and compatibility

The product layer should make this distinction explicit:

| Core concept | Possible Cloudflare mapping | Extra contract needed |
| --- | --- | --- |
| Stateless actor service | Worker request/event entrypoint | Trigger, response, binding scope, invocation lifetime, external-operation failures |
| Keyed stateful actor | Durable Object per key | Identity routing, persistent vs volatile state, input/output gates, storage transactions, suspension/restart semantics |
| One-way delivery | Queue producer/consumer | At-least-once delivery, retries, attempts, acknowledgments, duplication and limits |
| Relational/object/key-value storage | D1 / R2 / KV | Separate typed operations and consistency profiles; not generic actor memory |

These are **adapters**, not synonyms. A deployment target, when one exists, must never assume a generic `stateful` actor is automatically durable or that generic calls are queues. Continue to reject unimplemented semantics at elaboration time.

The existing `cf-core-v0` Worker/D1 sources remain regression fixtures via the transitional parser shim, while the new spike uses `actors-v0`. Do not change their declared meaning in place. The shim is provisional: prior to merging decide whether to (a) migrate examples with an automated diagnostic, (b) retain `worker` only in a Cloudflare frontend, or (c) support both under explicitly versioned profiles. New function/actor source cannot select `cf-core-v0`. The spike's trace format version changed from 1 to 2 to account for actor state/frames; old artifacts are refused. Further layout or semantic changes must likewise version/reject or explicitly migrate replay artifacts. Avoid publishing a successful check under a profile whose resource behavior the checker cannot implement.

### Security, privacy and observability

Model source is untrusted and cannot invoke host Rust or network libraries. Guard nesting, expanded function cost, state size, call depth and artifact size; keep source-span diagnostics and deterministic replay. Never permit an `Actor<State>` capability to escape through a seemingly generic value. Traces may contain modeled user data and state: use synthetic data, escape control characters and retain no telemetry by default. Reports list omitted failures and durability assumptions, so a result cannot be mistaken for an implementation proof.

### Rollout and validation

Continue work **on `spike/actor-generalization`** in end-to-end increments:

1. Keep the [existing suite](../../tests/) green and replay old Worker/D1 fixtures under `cf-core-v0`. Check both [`actor-stateless.fml`](../../examples/actor-stateless.fml) and [`actor-counter.fml`](../../examples/actor-counter.fml) via CLI; the latter intentionally fails and replays. Improve the `actors-v0` report so it does not imply Cloudflare storage semantics when no such resource appears.
2. Test the core: pure helper substitution vs inline computation; effectful helper suspension at D1; rejected capability escape through every available type constructor/alias/list; tests for matching, scope, recursion, unsupported calls and vacuous claims. Preserve file/line/column for nested call actions.
3. Add keyed address/instance semantics and property-only keyed inspection, a finite scope declaration, and same-key/different-key fixtures. Include a counterexample for a race across suspension within one actor. Do not infer durability.
4. Add direct actor request/reply calls with pending correlated frames, independently scheduled target, fair-lasso coverage and replay. Exercise missing handler/wrong payload errors, self/cyclic calls, deadlock, scope overflow, orphaned calls (where relevant) and source-level trace causality. Version the new semantic profile and artifact if behavior changes.
5. After the generic semantics pass, add *separate* Cloudflare adapters, starting with a Worker/DO contrast. Specify omissions and product litmus tests first. Queue delivery and D1 transaction behaviors follow RFD0001's distinct contracts rather than being folded into `actor`.

Required checks at each step: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, CLI `fml check` of passing/failing `.fml` fixtures, and `fml replay` of counterexamples. Expand fuzz targets and independently cross-check temporal behavior when adding new scheduler actions. A new syntax feature that parses but lacks typed execution/checking must fail explicitly until implemented. Before declaring the actor model stable, run both generic and Cloudflare-specific example suites with accurate profile labels, resource budgets and unsupported-feature diagnostics.

## Drawbacks

- A generic actor core increases the surface to specify and validate; the attractive syntax may invite overconfidence in a simplistic mailbox model.
- Separating product adapters risks another layer of versioning and trace compatibility work. The generic `Actor<State>` capability must not silently accumulate backend-specific guarantees.
- Supporting keyed instances and actor-to-actor calls can explode the state space and needs robust capacity accounting and fairness/correlation tests.
- Source that resembles normal functions can be mistaken for executable application code. The CLI and docs must keep saying *model*, not deploy.
- A transitional legacy syntax shim burdens parser/typechecker maintenance until a migration decision is made.
- Rejecting advanced higher-order or recursive functions limits expressiveness, but allowing unsupported control flow would invalidate verification.

## Rationale and alternatives

### Proposed design

A small language core describes typed computation and state ownership; product profiles describe resource and failure behavior. This reuses the native checker and permits systems unrelated to Cloudflare without disguising how Cloudflare primitives differ. The two runnable spike examples demonstrate that functions and actors compose with existing invariants, temporal claims and replay; the target contract closes the spike's gaps before claiming generality.

### Simpler or narrower approach

Keep `worker` as the only handler declaration and introduce only pure helper functions. This would improve reuse quickly but leave state identity and communication tied to product syntax. It remains an option if typed actor calls/ownership do not justify the complexity; the current spike gives us a way to measure that tradeoff.

### Other alternatives considered

- **Everything is a function, no actor declaration:** cannot determine which calls create independent invocations, own state, suspend, retry or expose an address. Those semantics would become magic conventions.
- **Rename `worker` to `actor`, preserve singleton state:** minimal parser change but incorrect for keyed identity, message boundaries and interleaving. Do not claim a DO mapping from that rename.
- **Universal mailbox backed by queues:** confuses direct request/reply with asynchronous at-least-once delivery and would hide retries and duplicates.
- **A completely user-extensible semantic library now:** appealing for generality, but without a trusted, versioned semantic contract it could manufacture misleading guarantees. Keep extensions explicit and tested first.

### Do nothing

Continue RFD0001's product-first model. It checks useful Cloudflare systems, but non-Cloudflare modeling remains awkward and new backend behavior risks being encoded as more keywords instead of clear compositional semantics.

## Prior art

- [RFD0001](RFD0001-initial-language-and-model-checker.md): keep its finite checking, fairness and evidence rules. This RFD proposes replacing its core `worker`/`durable` surface with functions and actors; it **does not** erase its differentiated Cloudflare resource semantics or pretend the first release is complete.
- [Actor spike notes](../spikes/actor-generalization.md) and [`src/functions.rs`](../../src/functions.rs): provide concrete proof of a CLI vertical slice and reveal missing address, capability and effect guarantees. An executable spike is evidence, not a final specification.
- [Riot](https://github.com/leostera/riot-lang): ML-shaped types and actor vocabulary are a useful syntactic precedent; process supervision (`link`/`monitor`) cannot be copied verbatim into a Cloudflare runtime model.
- [TLA+](https://lamport.azurewebsites.net/tla/tla.html): system state, actions, fairness and temporal properties remain the semantic touchstones. A pleasant syntax does not replace explicit assumptions.
- [Durable Object rules](https://developers.cloudflare.com/durable-objects/best-practices/rules-of-durable-objects/): single-threaded objects can interleave across external awaits; persistent storage and output gates are extra product semantics, not generic actor features.

## Unresolved questions

### Before acceptance

- Should keyed identity be mandatory for all stateful actors (`Unit` for a singleton), and should the surface be `Account.at(id)`, `Account[id]`, or another address form? Require a precise typed addressing rule, not just aesthetics.
- Should `Actor<State>` be an explicit parameter, as in the spike, or implicitly scoped in actor-bound functions? How do we prevent capability escape in every supported data type?
- What is the smallest useful `call` failure profile: no faults first, or an explicit timeout/unknown-outcome branch from the start? State the omission prominently either way.
- Is the transitional `worker` syntax a compatibility frontend, a short-lived migration shim, or removed before an actor release? Existing `cf-core-v0` files and archived traces must not silently change meaning.

### During implementation

- Define stable invocation/call/continuation IDs, actor-state canonicalization, weak fairness action IDs and capacity behavior for cycles and keyed instances.
- Choose a lower-level IR for local calls and effectful suspension that retains source spans without interpreting recursive AST bodies indefinitely.
- Separate capability/serialization checking from ordinary data type resolution and add focused fuzz/property tests; no untyped fallback.
- Decide when to introduce a formal Cloudflare adapter registry vs keeping a versioned built-in D1 module.

### Out of scope

- Verified code generation or conformance testing against a production Worker/DO implementation.
- Universal remote actor protocols, fault-tolerant messaging, restart recovery or durable state without a separate adapter/profile.
- Arbitrary custom temporal operators, unbounded verification or recursive higher-order functions.

## Future possibilities

A composable library of **trusted semantic adapters** could model payments, replicated stores or other runtimes alongside Cloudflare; a verified generic actor core would make those additions approachable. Richer temporal composition, symmetry/partial-order reductions and graphical trace exploration are valuable later, but cannot justify weakening today's exact finite-check results or pretending that a well-typed model matches deployed code.
