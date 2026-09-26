# RFD0002 - Functions and actors as the modeling core

- Feature Name: `functions-and-actors`
- Status: Draft
- Mode: Proposal
- Author: leostera, with AI assistance
- Start Date: 2026-09-26
- Updated: 2026-09-26
- Implementation: [actor-generalization spike](../spikes/actor-generalization.md) on `spike/actor-generalization` implements both the earlier synchronous `actors-v1` design and an **experimental finite, fault-free `actors-v2` slice** of this proposal. The full RFD remains unimplemented.

## Summary

Use one `actor` declaration for both actors with and without retained state. A stateful actor's `init(args)` produces its initial state; its `handle_message(state, message)` computes the next state. Stateless actors omit `init` and the state argument. Typed `send(address, message)` enqueues a one-way message and returns no reply. A caller that wants a response includes its typed address and a finite correlation ID in the message; the recipient sends a separate response. A callback's state change and outgoing messages commit together, before the recipient can process any of those messages. The first generic messaging profile has finite per-address mailboxes, no loss, duplication, transport faults, restarts, or durability. Fairness and capacity are explicit. Cloudflare Worker, Durable Object, Queue, and storage behaviors require **separate semantic adapters**; none follow from `actor` or `send`. Preserve the finite native checker, temporal properties, source-level witnesses and replay from [RFD0001](RFD0001-initial-language-and-model-checker.md).

This RFD remains a **Draft**, not a stable contract. `actors-v0`, `actors-v1`, and `cf-core-v0` remain pinned to their existing meanings. The branch now implements a distinct experimental `actors-v2` slice with typed messages, finite FIFO mailboxes, bounded atomic callback commits, fairness, and replay. Dynamic-message temporal observations, fault/restart behavior, adapters, and independent validation remain unfinished.

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

### Actors receive messages and return state

**Working example on the spike branch** (also in [`examples/actor-messages.fml`](../../examples/actor-messages.fml)). Types and addresses are finite, and `send` is not a function that waits for a result:

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

check OneIncrement {
  semantics = "actors-v2"
  domain Int = 0..1
  mailbox_bound = 2
  inputs { once send(Counter.at(Main), Inc(Client.at(User), First)) }
  fairness { weak runtime.progress }
}
```

`Counter.at(Main)` and `Client.at(User)` are typed `Address<Counter>` and `Address<Client>` values. `send` accepts the target's message type, not any data. The client reply is a second message, not the return value of `Inc`. `RequestId` is ordinary finite user data used for correlation: the checker never assumes an address identifies one outstanding request. A stateless actor omits `init` and handles one message without a state parameter (its callback returns `unit`). An actor with state exposes read-only `.state` to specifications only, not to another actor's handler.

An external `once send(...)` is an optional, finite input. It can be chosen at most once; fairness does **not** force the environment to submit it. After it is submitted, fairness can require continuously enabled internal delivery/processing actions to progress. The cover demonstrates that the reply is reachable; it does not assert that optional input always arrives. To claim conditional liveness we need a typed view of submitted inputs; the exact temporal surface must be implemented and tested, not inferred from the old `requests(Actor.method)` inspector.

### When does `send` happen?

Inside a callback, `send` records an outgoing intent. It does not hand control to the target immediately. At the callback's return transition the checker validates the next state and **atomically** installs the new owned state and enqueues the staged messages, in program order. The target can first process the reply in a later transition, after the counter state is committed. If the callback cannot finish (type error or finite capacity cutoff), it does not partly commit state or partly enqueue messages. This is a **generic model rule**, not a claim that any particular external transport provides an atomic transaction.

The initial profile rejects external I/O or suspension within a state-transition callback. Such operations need an explicit continuation/state-machine protocol or a separately specified adapter. This avoids claiming that a callback can hold a stale `state` argument across an `await` and still commit atomically. Multiple actors' ready messages may be scheduled in either order; within one actor identity, only one message turn executes at a time. There is no implicit synchronous `call` in this profile.

## Reference-level explanation

### Language surface, typing and ownership

- A stateful `actor A(id: Key)` defines `init(id: Key): State` and one `handle_message(state: State, message: Message): State` in the initial profile. The key and message domains must be finite; a singleton is an actor keyed by `unit` (whether its key is written explicitly is a surface decision). `init` is pure, deterministic and evaluated once per finite address at check initialization. General `init(args)` beyond identity may be added only when the check supplies typed finite arguments and their lifecycle is specified.
- A stateless `actor A` defines `handle_message(message: Message): unit`; each accepted message runs with fresh locals. There is no retained state view. Additional handler methods/protocol variants are a future syntactic question: the initial message type can be an algebraic union and matched in one callback.
- `state` is a **value parameter** for a callback, not a live state handle or a mutable global. A callback returns its next state. Ordinary pure `let` functions may compute portions of that transition. `send` is an explicit effect allowed only in callback statements (and helpers with conservatively inferred send effects); it returns `unit`. A pure helper or property may not send. Direct local recursion remains rejected in the first finite implementation.
- `Address<A>` contains a typed actor name and finite key and may appear in records, variants, parameters and messages; it cannot grant access to `A`'s state. `send(address, message)` statically checks the actor's declared message protocol, including transitive data serializability. Owner capabilities from `actors-v1` cannot be serialized or smuggled through constructors, aliases, collections or helper returns. The new profile does not provide an owner capability to callbacks.
- Local code evaluation cannot invoke the host network or Rust APIs. An unsupported effect or message form is a source error, not a request silently executed as a pure function. `init` cannot send. Every reachable callback branch returns one typed next state (or `unit` for stateless actors).

### Mailboxes, turns and atomicity

- Each finite actor address has an ordered pending mailbox. An external accepted input or a successful callback commit appends one message per `send`. Enqueue transitions from different actors may interleave nondeterministically; once appended, FIFO order is preserved **per target address**. The order of sends from one callback to the same address is source program order. No global ordering across distinct addresses is asserted.
- A target may start only the head of its mailbox. In the initial profile, a complete callback (including local pure function calls and staged sends) is one atomic state transition: dequeue the head, compute and validate the next state and outbox, commit state, and append the outbox. Messages to self are appended after the active message is removed. A handler cannot process another message from its own address while its callback is running. Other addresses may proceed before or after this transition.
- A direct external input acceptance only appends to the mailbox; it does not also run the callback. This preserves a checkable boundary between submission and processing. The initial profile excludes transport loss, duplication, timeout and crash; **absence of faults is an explicit reported assumption**, not exactly-once delivery from Cloudflare Queues or an end-to-end guarantee.
- If processing or enqueuing would exceed a declared finite mailbox/message/turn bound or escape a finite data domain, return `INCONCLUSIVE` with the source operation and bound. Never drop the message, wrap a count, or treat a truncated graph as complete. Declare bounds in the check and trace; choose defaults only with measured fixtures. A bounded run may prove a property only for the bounded workload and state graph.
- A reply is just another `send` with a typed reply address. It may be processed only after its sender's commit. Correlation IDs are part of the modeled protocol; mismatched, duplicate and stale replies require model logic or specific failure semantics. There is no auto-generated waiting caller or response slot in `actors-v2`.

### Scheduling, fairness, properties and replay

- Nondeterministic choices include optional external inputs and enabled mailbox-head processing for each address. `weak runtime.progress` applies to an individual continuously enabled internal processing action. It does not force optional external submission. FIFO and atomic turns mean the head cannot be skipped by a later message; an empty mailbox does not give rise to a fairness obligation. A self-sending cycle remains a real infinite path, subject to finite bounds and fair scheduling; a permanently blocked turn must not be misreported as merely unfair.
- Preserve invariants at initial and post-transition states, including the state after enqueue but before dequeue and after a sender commits but before a recipient runs. Check liveness over fair infinite paths with replayable lassos. External input slots require observations such as `submitted`/`processed`; internal messages require a separate, bounded, well-defined observation mechanism. The existing `requests(Actor.method)` view is **not** silently reinterpreted to include dynamic messages; define any temporal quantification over messages explicitly and test non-vacuity.
- State hashing and serialized traces include keyed state, ordered mailbox contents, input-slot status, correlation-bearing messages, profile and limits, action labels and source/check identity. Retain distinct enabled edges even if they lead to equal data states. Replay re-executes each submission and callback commit, verifies ordering, state/outbox changes, source identity, loop closure and fairness. Trace layout changes require a format bump or explicit migration; old artifacts must not be silently accepted under new semantics.

### Product boundaries and compatibility

| Generic model concept | Possible Cloudflare mapping | Separate contract required |
| --- | --- | --- |
| Stateless actor / message turn | Worker event entrypoint | Trigger type, invocation lifetime, response, bindings and failure behavior |
| Keyed stateful actor | Durable Object by key | Routing, persistent versus volatile state, storage transactions, gates, suspension and restart |
| Generic `send` / mailbox | **Not** automatically a Queue | Delivery attempts, duplicates, acknowledgment, retries, exhaustion and batch behavior |
| Typed storage operations | D1 / KV / R2 | Distinct schemas, transactions, consistency and visibility |

A generic mailbox is an in-memory modeling abstraction, not durable actor storage and not a Queue producer/consumer. Backend profiles must specify how a callback and outbox interact with external side effects and failures; if atomic state-and-send commit is not supported by a product, the adapter **must split that operation into truthful transitions** rather than inherit the generic rule. A model may be checked against its declared generic assumptions without implying production conformance.

The implementation has `actors-v0` functions/actors, `actors-v1` finite keyed actors and synchronous fault-free `call` with suspended callers, a `cf-core-v0` Worker/D1 parser shim, and a **separate experimental `actors-v2` slice** with inline handlers, one-way `send`, ordered finite mailboxes, and replay. The older profiles remain regression profiles; do not silently reinterpret `call` as `send` or `owner.set` as a returned next state. Decide separately whether to migrate legacy source, keep `worker` as a Cloudflare-only frontend, or retire it with diagnostics. The v2 trace uses format version 4; older profiles retain format 3, and artifacts are checked against their declared profile.

### Security, privacy and observability

Models cannot execute arbitrary host code or network operations. Enforce typed addresses, closed finite message data and non-escaping capabilities at every boundary. Guard parser nesting, elaboration, mailbox/message capacity, generated sends, graph size and artifact size. A trace can contain synthetic modeled user data, including reply addresses and messages; escape control characters in terminal output and do not collect telemetry by default. Reports prominently list fault, storage and delivery omissions.

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

### Before acceptance

- Confirm the chosen staged-send/atomic-commit abstraction for the generic profile and the FIFO-per-address rule. These are **proposed modeling assumptions**, not Cloudflare claims; choose different explicit profiles if a use case needs weaker guarantees.
- Is `init(id)` enough for the first finite profile, or must `init(args)` accept check-supplied configuration? If so, define exactly which finite arguments each address receives and when they are evaluated.
- Should stateless callbacks take only `(message)` and return `unit`, or should every actor have a uniform `unit` state parameter? This draft proposes the former.
- Is one `handle_message` method over an algebraic message type sufficient initially? How should errors or rejected messages appear in the protocol rather than being silently swallowed?
- Decide whether a syntactic singleton omits its `unit` key, whether `Address<ActorName>` remains the address type, and whether the old `worker` shim becomes a Cloudflare frontend, a migration tool, or is removed.

### During implementation

- Specify input and internal-message observation types, including temporal quantification over dynamically generated messages without checking only an initial snapshot.
- Specify source-stable action IDs, mailbox/message canonicalization, bound accounting, replay format and fairness for identical messages and send-to-self cycles.
- Audit transitive send effects, constructor/alias/collection non-escape, integer domains, early returns, match exhaustiveness and malformed artifacts. Add independent tiny-model oracles before calling the profile stable.
- Decide where backend adapters are registered versus keeping D1 built in under `cf-core-v0`.

### Out of scope

- Production code generation or conformance proof, supervision/monitoring, alarms, arbitrary higher-order functions and unbounded model checking.
- A Cloudflare Worker/DO/Queue equivalence claim, durable outbox, transport retry/uncertain delivery or synchronous RPC without a separately specified backend/profile.

## Future possibilities

A typed `request`/`reply` library could synthesize addresses and correlation IDs over `send` once its timeout, duplicate and failure semantics are explicit. Backend adapters could add DO persistence/gates, Queue retries or other systems without disguising their guarantees as the generic actor core. Richer temporal quantification, symmetry and partial-order reductions remain useful later, but none justify silently weakening today's exact finite-check results.
