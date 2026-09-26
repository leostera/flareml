# RFD0002 - Functions and actors as the modeling core

- Feature Name: `functions-and-actors`
- Status: Draft
- Mode: Proposal
- Author: leostera, with AI assistance
- Start Date: 2026-09-26
- Updated: 2026-09-26
- Implementation: [actor-generalization spike](../spikes/actor-generalization.md) on `spike/actor-generalization` implements the generic `actors-v2` core, including bounded message observations and independent scheduler tests; the earlier synchronous `actors-v1` remains a regression profile. [Acceptance checklist](RFD0002-implementation-checklist.md) records completed core work and remaining adapter/release gates.

## Summary

Use one `actor` declaration for both actors with and without retained state. A stateful actor's `init(args)` produces its initial state; its `handle_message(state, message)` computes the next state. Stateless actors omit `init` and the state argument. Typed `send(address, message)` enqueues a one-way message and returns no reply. A caller that wants a response includes its typed address and a finite correlation ID in the message; the recipient sends a separate response. A callback's state change and outgoing messages commit together, before the recipient can process any of those messages. The first generic messaging profile has finite per-address mailboxes, no loss, duplication, transport faults, restarts, or durability. Fairness and capacity are explicit. Cloudflare Worker, Durable Object, Queue, and storage behaviors require **separate semantic adapters**; none follow from `actor` or `send`. Preserve the finite native checker, temporal properties, source-level witnesses and replay from [RFD0001](RFD0001-initial-language-and-model-checker.md).

This RFD remains a **Draft**, not a stable product-adapter contract. `actors-v0`, `actors-v1`, and `cf-core-v0` retain their semantics. The generic `actors-v2` implementation now includes typed messages, finite FIFO mailboxes, atomic callback commits, input and bounded lifetime message observations, fairness, replay, and an independent mailbox scheduler oracle. The adapter gate below is still outstanding, and coverage-instrumented fuzzing remains a release task. Faults and restarts are explicitly excluded from this generic profile, not silently approximated or claimed complete.

## Motivation

The initial Worker/D1 checker treats product-named handlers as the whole modeling vocabulary. The [spike](../spikes/actor-generalization.md) introduced typed functions, `stateless actor` and `stateful actor`, a scoped `Actor<State>` capability, finite keyed identities and `call(address.method, message)` with a waiting caller. This works as an experimental vertical slice, but it makes state ownership look like a different *kind* of actor, and presents direct synchronous RPC as the general communication primitive. That is a poor fit for message-driven systems where a request can carry a reply address and the sender need not wait.

The desired mental model is closer to an asynchronous receive loop: the actor owns one state per identity, processes messages under a typed protocol, and explicitly sends further messages. A handler is a state transition, not an application implementation or an automatically atomic distributed transaction. The language should make reply routing, message correlation, state commits, and assumptions about progress visible to the checker.

## Goals

- One unbranded `actor` form. State ownership is declared by `init`; an actor without `init` has independent stateless message invocations.
- A typed finite address and message protocol. An address can be passed in ordinary model data; an owned state value/capability cannot escape in a message.
- A handler that consumes a state snapshot and message, returns the next state, and can stage explicit one-way `send` effects. No implicit response or hidden state mutation.
- Specify per-address mailbox ordering, delivery, callback atomicity, backpressure/capacity, scheduling, fairness, failure omissions, and reply-before-commit behavior precisely.
- Preserve distinct Cloudflare resource contracts and the checker's honest `VERIFIED_IN_SCOPE` / `VIOLATED` / `INCONCLUSIVE` results and replayable evidence.
- Migrate without reinterpreting legacy source profiles or archived traces.

## Non-goals

- An automatically generated `handle_call` RPC or reply channel in the first asynchronous profile. Application protocols carry reply addresses and correlation IDs explicitly.
- Runtime deployment, automatic conformance with production code, supervision, process linking, arbitrary spawning, timers, crashes, or retry semantics in the generic profile.
- Treating generic actor mailboxes as Cloudflare Queues, or retained generic state as Durable Object storage.
- Unbounded identities, messages, mailboxes, recursion, or proofs. A finite bound is a modeling scope, not a production guarantee.
- Changing the meaning of existing `actors-v0`, `actors-v1`, or `cf-core-v0` files under the same profile string.

## Guide-level explanation

### Actors are a computational abstraction

An actor is a participant in the model, not a choice of deployment technology. Its role might be implemented by an Erlang recursive receive loop, a Cloudflare Worker invocation, a containerized service, or a thread with a mailbox. Functions describe behavior; actors delimit identity, protocols and optional owned state. The implementation's real scheduling, delivery, storage and failure behavior must still match the selected semantic profile: a common actor vocabulary does not make these implementations equivalent. In particular, modeling a Worker invocation as a participant does not give Workers persistent per-actor state or the generic profile's atomic state-and-send commit.

Examples are named for their scenarios—counter replies, missing replies, account isolation and lost updates—not for the `actor` language construct. The [scenario guide](../../examples/README.md) distinguishes current message-based models from older synchronous and resource-specific regression profiles.

### Actors receive messages and return state

**Working example on the spike branch** (also in [`examples/counter-replies.fml`](../../examples/counter-replies.fml)). Types and addresses are finite, and `send` is not a function that waits for a result:

```fml
type CounterId = Main
type ClientId = User
type RequestId = First

type CounterMessage = Inc(Address<Client>, RequestId)
type ClientMessage = Counted(RequestId, Int)
type ClientState = Waiting | Observed

actor Counter(id: CounterId) {
  init(id: CounterId): Int { 0 }

  handle_message(state: Int, message: CounterMessage): Int {
    match message {
      | Inc(reply_to, request_id) -> {
          let next = state + 1
          send(reply_to, Counted(request_id, next))
          next
        }
    }
  }
}

actor Client(id: ClientId) {
  init(id: ClientId): ClientState { Waiting }

  handle_message(state: ClientState, message: ClientMessage): ClientState {
    match message {
      | Counted(_, _) -> Observed
    }
  }
}

invariant "a reply is not processed before the counter commits" {
  Client.at(User).state == Observed implies Counter.at(Main).state == 1
}
cover "client receives a reply" { Client.at(User).state == Observed }

property "submitted increments eventually receive a reply" {
  forall (i in inputs(Counter)) {
    i.submitted leads_to Client.at(User).state == Observed
  }
}
property "generated replies are eventually processed" {
  forall (m in messages(Client)) { m.sent leads_to m.processed }
}

check OneIncrement {
  semantics = "actors-v2"
  domain Int = 0..1
  mailbox_bound = 2
  message_bound = 2
  inputs { once send(Counter.at(Main), Inc(Client.at(User), First)) }
  fairness { weak runtime.progress }
}
```

`Counter.at(Main)` and `Client.at(User)` are typed `Address<Counter>` and `Address<Client>` values. `send` accepts the target's message type, not any data. The client reply is a second message, not the return value of `Inc`. `RequestId` is ordinary finite user data used for correlation: the checker never assumes an address identifies one outstanding request. A stateless actor omits `init` and handles one message without a state parameter (its callback returns `unit`). An actor with state exposes read-only `.state` to specifications only, not to another actor's handler.

An external `once send(...)` is an optional, finite input. It can be chosen at most once; fairness does **not** force the environment to submit it. After submission, fairness requires continuously enabled mailbox processing to progress. `inputs(Counter)` exposes those input slots; `messages(Client)` includes replies generated later. The cover establishes reachable reply processing, while the properties establish conditional liveness under the declared fairness and finite bounds. Removing fairness produces replayable starvation lassos. `requests(Actor.method)` retains its old meaning in older profiles and is rejected in `actors-v2`.

### When does `send` happen?

Inside a callback, `send` records an outgoing intent. It does not hand control to the target immediately. At the callback's return transition the checker validates the next state and **atomically** installs the new owned state and enqueues the staged messages, in program order. The target can first process the reply in a later transition, after the counter state is committed. If the callback cannot finish (type error or finite capacity cutoff), it does not partly commit state or partly enqueue messages. This is a **generic model rule**, not a claim that any particular external transport provides an atomic transaction.

The initial profile rejects external I/O or suspension within a state-transition callback. Such operations need an explicit continuation/state-machine protocol or a separately specified adapter. This avoids claiming that a callback can hold a stale `state` argument across an `await` and still commit atomically. Multiple actors' ready messages may be scheduled in either order; within one actor identity, only one message turn executes at a time. There is no implicit synchronous `call` in this profile.

## Reference-level explanation

### Language surface, typing and ownership

- A stateful `actor A(id: Key)` defines `init(id: Key): State` and one `handle_message(state: State, message: Message): State` in the initial profile. The key and message domains must be finite. A singleton omits `(id: Key)`, uses `init(): State`, and its declaration name is its typed address; keyed actors use `A.at(key)`. `init` is pure, deterministic and evaluated once per finite address at check initialization, after all addresses are materialized. Its parameter name may differ from the declaration's key name; transparent aliases are allowed. General check-supplied `init(args)` configuration is deferred to a separate extension; the implemented core intentionally supports identity-only initialization.
- A stateless `actor A` defines `handle_message(message: Message): unit`; each accepted message runs with fresh locals. There is no retained state view. Additional handler methods/protocol variants are a future syntactic question: the initial message type can be an algebraic union and matched in one callback.
- `state` is a **value parameter** for a callback, not a live state handle or a mutable global. A callback returns its next state. Ordinary pure `let` functions may compute portions of that transition. `send` is an explicit effect allowed only in callback statements (and helpers with conservatively inferred send effects); it returns `unit`. A pure helper or property may not send. Direct local recursion remains rejected in the first finite implementation.
- `Address<A>` contains a typed actor name and finite key and may appear in records, variants, parameters and messages; it cannot grant access to `A`'s state. `send(address, message)` statically checks the actor's declared message protocol, including transitive data serializability. Owner capabilities from `actors-v1` cannot be serialized or smuggled through constructors, aliases, collections or helper returns. The new profile does not provide an owner capability to callbacks.
- Local code evaluation cannot invoke the host network or Rust APIs. An unsupported effect or message form is a source error, not a request silently executed as a pure function. `init` cannot send. Every reachable callback branch returns one typed next state (or `unit` for stateless actors).

### Mailboxes, turns and atomicity

- Each finite actor address has an ordered pending mailbox. An external accepted input or a successful callback commit appends one message per `send`. Enqueue transitions from different actors may interleave nondeterministically; once appended, FIFO order is preserved **per target address**. The order of sends from one callback to the same address is source program order. No global ordering across distinct addresses is asserted.
- A target may start only the head of its mailbox. In the initial profile, a complete callback (including local pure function calls and staged sends) is one atomic state transition: dequeue the head, compute and validate the next state and outbox, commit state, and append the outbox. Messages to self are appended after the active message is removed. A handler cannot process another message from its own address while its callback is running. Other addresses may proceed before or after this transition.
- A direct external input acceptance only appends to the mailbox; it does not also run the callback. This preserves a checkable boundary between submission and processing. The initial profile excludes transport loss, duplication, timeout and crash; **absence of faults is an explicit reported assumption**, not exactly-once delivery from Cloudflare Queues or an end-to-end guarantee.
- `mailbox_bound = N` is required (`1..4096`) and bounds pending envelopes per address. Optional `message_bound = N` (`1..4096`) bounds lifetime observation slots **per actor declaration across all its keys**, including external sends; it is not a transport retry policy or a mailbox size. If any transition would exceed a capacity, return `INCONCLUSIVE` at the source operation, never disable the action, partly commit, reuse a slot, or drop a message. Bounds appear in trace metadata and reports. With no lifetime observation bound, finite-state self-sending cycles can still be checked without retaining unbounded history. Graph/state/depth/time budgets remain CLI exploration guards, not definitions of eventuality.
- A reply is just another `send` with a typed reply address. It may be processed only after its sender's commit. Correlation IDs are part of the modeled protocol; mismatched, duplicate and stale replies require model logic or specific failure semantics. There is no auto-generated waiting caller or response slot in `actors-v2`.

### Scheduling, fairness, properties and replay

- Nondeterministic choices include optional external inputs and enabled mailbox-head processing for each address. `weak runtime.progress` applies to an individual continuously enabled internal processing action. It does not force optional external submission. FIFO and atomic turns mean the head cannot be skipped by a later message; an empty mailbox does not give rise to a fairness obligation. A self-sending cycle remains a real infinite path, subject to finite bounds and fair scheduling; a permanently blocked turn must not be misreported as merely unfair.
- Preserve invariants at initial and post-transition states, including after enqueue but before dequeue and after a sender commits but before a recipient runs. Check liveness over fair infinite paths with replayable lassos. The typed observation views below are read-only and specification-only. The existing `requests(Actor.method)` view is **not** silently reinterpreted.
- State hashing and serialized traces include keyed state, ordered mailbox contents, input-slot status, correlation-bearing messages, profile and limits, action labels and source/check identity. Retain distinct enabled edges even if they lead to equal data states. Replay re-executes each submission and callback commit, verifies ordering, state/outbox changes, source identity, loop closure and fairness. Trace layout changes require a format bump or explicit migration; old artifacts must not be silently accepted under new semantics.

### Stable observations and temporal bindings

`inputs(A)` ranges over the declared external input slots for **all keys** of actor declaration `A`. Each slot has a typed `payload: Message`, `target: Address<A>`, `submitted: Bool`, and `processed: Bool`. The two flags start false and remain true once set. Submission enqueues the envelope; processing marks the slot only when its target callback commits. Processing does **not** mean a reply or any generated work was processed. A caller must express that condition using its reply protocol/state or a message observation.

`messages(A)` requires `message_bound = N` and always ranges over **N stable slots**, including at initialization when no sends have occurred. Each slot has `sent`, `processed`, and `external` Boolean fields plus `payload: Option<Message>` and `target: Option<Address<A>>`. Before allocation, flags are false and the optional fields are `None`. Each enqueue allocates the next unused slot for that actor declaration, records immutable payload/target and whether it originated from an external input, and sets `sent`. Its callback commit sets `processed`. Identical payloads sent twice get distinct slots. Slots are never recycled, including after processing: otherwise a temporal binding could silently change identity. Exhaustion is inconclusive, even if every older message finished.

Temporal `forall` expands over these **stable potential slots**, not the messages already present in the initial state. Thus `forall (m in messages(A)) { m.sent leads_to m.processed }` covers later internal sends rather than proving an empty quantifier. Covers establish reachable antecedents. Unused slots can produce an unreached-antecedent note; this is distinct from an empty quantifier. The lifetime bound deliberately cannot prove unbounded-message protocols by silently truncating history; unobserved finite-state cycles and input-based liveness remain available without it. Observers cannot be used in handlers or initializers, stored in user data, or sent to another actor. [`missing-reply.fml`](../../examples/missing-reply.fml) demonstrates why processed work is not an implicit reply: every counter message finishes, but the client still has a replayable fair liveness failure.

### Product boundaries and compatibility

| Generic model concept | Possible Cloudflare mapping | Separate contract required |
| --- | --- | --- |
| Stateless actor / message turn | Worker event entrypoint | Trigger type, invocation lifetime, response, bindings and failure behavior |
| Keyed stateful actor | Durable Object by key | Routing, persistent versus volatile state, storage transactions, gates, suspension and restart |
| Generic `send` / mailbox | **Not** automatically a Queue | Delivery attempts, duplicates, acknowledgment, retries, exhaustion and batch behavior |
| Typed storage operations | D1 / KV / R2 | Distinct schemas, transactions, consistency and visibility |

A generic mailbox is an in-memory modeling abstraction, not durable actor storage and not a Queue producer/consumer. Backend profiles must specify how a callback and outbox interact with external side effects and failures; if atomic state-and-send commit is not supported by a product, the adapter **must split that operation into truthful transitions** rather than inherit the generic rule. A model may be checked against its declared generic assumptions without implying production conformance.

The implementation has `actors-v0` functions/actors, `actors-v1` finite keyed actors and synchronous fault-free `call` with suspended callers, a `cf-core-v0` Worker/D1 parser shim, and a **separate experimental `actors-v2` slice** with inline handlers, one-way `send`, ordered finite mailboxes, and replay. The older profiles remain regression profiles; do not silently reinterpret `call` as `send` or `owner.set` as a returned next state. Keep the legacy `worker` frontend and synchronous actor profiles as frozen regression paths; they are not accepted as unified `actors-v2` declarations. Migration to v2 is explicit: replace mutable owner writes with a returned next state, replace synchronous calls with protocol messages/reply addresses, and reconsider invariants at the new atomic-turn boundaries. This is not a semantics-preserving text rewrite, so no automatic migration is offered. Trace format **5** stores envelope provenance, input completion and lifetime observation records plus both bounds; previous v2 format-4 artifacts must be regenerated and are rejected. Older profiles retain format 3.

### Security, privacy and observability

Models cannot execute arbitrary host code or network operations. Enforce typed addresses, closed finite message data and non-escaping capabilities at every boundary. Recursive data is rejected in v2 until an explicit depth-bound profile exists. Static guards include parser/tree nesting (128), data-type/call-graph depth (64), and expanded function cost (10,000 nodes). Runtime guards include evaluation nesting (128), individual value size/depth (4096 nodes/64 levels), and total actor addresses (4096), in addition to mailbox/history and CLI graph budgets. Runtime exhaustion is inconclusive; static unsupported syntax/elaboration is invalid. These are host-safety limits, not product guarantees. A trace can contain synthetic modeled user data, including reply addresses and messages; escape control characters in terminal output and do not collect telemetry by default. Reports prominently list fault, storage and delivery omissions.

### Implementation and validation

1. Preserve the existing `cf-core-v0`, `actors-v0` and `actors-v1` tests and trace rejection rules. Add **new failing parser/profile-gating tests** for proposed `actor`, `init`, `handle_message` and `send` forms under older profiles before implementing the new one.
2. Implement unified actor declarations, pure typed `init`, state-value callback parameters, full-branch next-state typing, and typed address/message protocols. Add stateless, singleton and keyed fixtures; reject calls to unavailable protocols and state/capability escape.
3. Add the mailbox/outbox state and source-ordered staged `send` effects; expose input acceptance and atomic callback commit as separate actions. Test two senders to one target, two target keys, send-to-self, replies with correlation IDs, the impossibility of a reply processing before the sender commits, capacity `INCONCLUSIVE` and forbidden I/O/suspension inside the callback.
4. Extend safety/temporal observations, fairness and replay for the new action families. Cross-check tiny graphs with an independent oracle and fuzz source/trace JSON. Include fair and unfair progress fixtures, a non-vacuous reply property and a replayable failed invariant. Validate colored/`NO_COLOR` CLI and ANSI-free JSON.
5. Only after generic semantics pass, design separate Cloudflare Worker/DO and Queue adapters with product litmus tests. Do not label the generic profile as implementing either adapter. Resolve the old `worker` shim and migration plan before declaring the core stable.

Run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, CLI checks of both passing and failing `.fml` fixtures, and replay after each end-to-end increment. Keep a new syntax feature rejected rather than accepted with unimplemented checking.

## Drawbacks

- Returning a new state instead of writing through a handle can make simple updates more verbose and makes a snapshot's meaning across future suspension particularly important. The initial profile therefore excludes suspension inside callbacks.
- Atomic state-plus-outbox commit is convenient for a generic finite model but can be mistaken for a guarantee from a real transport. Every backend must either justify it or model weaker boundaries.
- A FIFO mailbox with fault-free one-time processing is a narrow abstraction. Queue retries, distribution, actor restart and DO gating require other profiles; a generic name alone does not cover them.
- Explicit reply addresses and correlation IDs add model code. They reveal ordering and duplicate-response bugs that implicit RPC can hide, but may warrant a separately specified convenience layer later.
- Every finite address, message, queue position and possible send increases state space. Honest limits may make useful checks inconclusive.
- Versioning and preserving the synchronous `actors-v1` slice increases maintenance cost during migration.

## Rationale and alternatives

### Proposed design

One actor form keeps the public concept simple; `init` declares retained identity and state. State-in/state-out callbacks make ownership and commits explicit. One-way `send` gives asynchronous systems a composable primitive, while reply addresses and IDs express request/reply without baking a blocking RPC into the core. A precise generic mailbox is useful for modeling, but it is not a product adapter.

### Simpler local solution

Keep `actors-v1`, rename `stateless actor`/`stateful actor` to `actor`, and retain `call` plus `owner.set`. This would reduce parser churn and preserve tested code but would keep synchronous waiting and state-capability mutation as the dominant mental model. It remains a supported experimental profile, not the proposed new core.

### Other alternatives

- **Implicit mutable `state` inside a handler:** concise, but a bare expression such as `state + 1` does not say whether it changes owned state. An explicit returned next state makes that unambiguous.
- **Use `handle_call` with automatic reply tokens:** useful OTP precedent, but conflates the primitive actor mailbox with a synchronous protocol. Could be an explicitly versioned layer on top of messages, not a silent property of every actor.
- **Expose `send` immediately while a callback runs:** allows a target to act on a reply before the sender's new state is installed. More faithful to some runtimes, but harder to reason about; represent it in an adapter that splits publication and commit if needed.
- **No FIFO guarantee:** simpler scheduler and suitable for some transports, but too weak for a single-address receive loop. A separately versioned transport may weaken it.
- **Universal Cloudflare mailbox:** incorrectly equates Worker events, DO storage and Queue delivery. Shared checker machinery does not justify collapsing their public contracts.

### Do nothing

Keep the synchronous spike as the core. It is useful for fault-free direct calls and keyed state, but makes independent replies and asynchronous cycles look like special cases rather than ordinary modeled behaviors.

## Prior art and related work

- [RFD0001](RFD0001-initial-language-and-model-checker.md): retain the finite checker, fairness and differentiated Cloudflare resource contracts; revise only the generic actor vocabulary and messaging design.
- [Actor spike notes](../spikes/actor-generalization.md): `actors-v1` remains a keyed synchronous regression corpus; `actors-v2` has an experimental asynchronous end-to-end fixture. Neither is evidence of product-specific delivery semantics or completion of this RFD.
- [Erlang/OTP `gen_server`](https://www.erlang.org/doc/apps/stdlib/gen_server.html): state-in/state-out callbacks and distinct call/cast/info handling motivate the design. This proposal resembles asynchronous `cast`/message handling more than `handle_call`: OTP synchronous calls carry a reply token, not merely an actor address. FML does not claim OTP supervision or scheduling equivalence.
- [Riot](https://github.com/leostera/riot-lang): ML-shaped variants, functions and messaging inform the syntax; its process lifecycle is not automatically FML's semantic profile.
- [TLA+](https://lamport.azurewebsites.net/tla/tla.html): explicit state, actions, fairness and infinite behaviors remain the checker contract.
- [Durable Object rules](https://developers.cloudflare.com/durable-objects/best-practices/rules-of-durable-objects/) and [Queues delivery guarantees](https://developers.cloudflare.com/queues/reference/delivery-guarantees/): real backend interleaving/persistence and at-least-once delivery must be specified separately, not inherited from the generic mailbox.

## Unresolved questions

### Generic-core decisions implemented

- FIFO per address, one non-suspending turn at a time, with state and staged sends committed together. These are generic modeling assumptions, not Cloudflare claims.
- Identity-only `init`, with no arguments for singletons. Stateless callbacks take only the message and return `unit`; keyed stateless mailboxes are supported too.
- One exhaustive `handle_message` over an algebraic message type. Application rejection is explicit protocol data; unsupported types/effects and missing branches are errors, not discarded messages.
- `Address<ActorName>` remains the transferable reference. Singleton names are addresses. Legacy `worker` and synchronous profiles remain frozen regression frontends, with explicit non-mechanical migration.
- Stable input and optional lifetime message observations as specified above. Processing action IDs are derived from typed target addresses (not growing sequence numbers); FIFO plus weak fairness ensures each queued head progresses. Envelope source spans and observation/input identities are retained for replay.

### Remaining acceptance work

- The separate product-adapter gate in implementation step 5 is not implemented. Specify and independently validate Worker/DO suspension, storage/restart/gate rules and Queue attempt/ack/retry rules before introducing those profiles. Decide adapter registration then; D1 remains built in under `cf-core-v0`.
- Run coverage-instrumented source/replay fuzz campaigns on a nightly toolchain and review this profile before treating the draft as stable. The local smoke runs use stable libFuzzer binaries without coverage instrumentation and do not satisfy this gate.
- Independent mailbox and temporal oracles, negative effect/capability tests, and replay checks improve confidence but are not a proof of checker correctness. Keep their corpora reproducible as the language evolves.

### Out of scope

- Production code generation or conformance proof, supervision/monitoring, alarms, arbitrary higher-order functions and unbounded model checking.
- A Cloudflare Worker/DO/Queue equivalence claim, durable outbox, transport retry/uncertain delivery or synchronous RPC without a separately specified backend/profile.

## Future possibilities

A typed `request`/`reply` library could synthesize addresses and correlation IDs over `send` once its timeout, duplicate and failure semantics are explicit. Backend adapters could add DO persistence/gates, Queue retries or other systems without disguising their guarantees as the generic actor core. Richer temporal quantification, symmetry and partial-order reductions remain useful later, but none justify silently weakening today's exact finite-check results.
