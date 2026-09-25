# Actor generalization — checkpoint / pickup notes

Branch: `spike/actor-generalization`, based on the initial `main` checkpoint `327bd44`. This is a **spike**, not a finished replacement for [RFD0001](../rfds/RFD0001-initial-language-and-model-checker.md). No deployable runtime is produced.

## RFD0002 implementation checkpoint (experimental `actors-v1`)

The sections below document the **original `actors-v0` checkpoint**; their "no calls/keyed instances" warnings are historical for that profile. The branch now has a second, opt-in `actors-v1` profile with typed `call(Actor.method, message)`, suspended callers, independently scheduled callee acceptance and reply, and finite keyed stateful actors addressed with `Actor.at(key)`. `Address<ActorName>` values can now be stored, sent, and used as `call(address.method, message)` without exposing the `Actor<State>` owner capability. Keyed input slots use `once Actor.at(key).method(message)`, and read-only specifications use `Actor.at(key).state`. Initial state is eagerly materialized for each finite key. `requests(Actor.method)` still observes only declared external slots. There are runnable passing and failing examples in `examples/actor-keyed.fml`, `examples/actor-address.fml`, `examples/actor-call.fml`, and `examples/actor-interleaving.fml`; the same-key interleaving example witnesses a lost update across an actor call. Replay artifacts are now version 3. `actors-v0` and `cf-core-v0` still run, and actor calls/keyed actors are gated to `actors-v1`.

**Still outstanding for RFD0002:** configurable call failure outcomes, restart/eviction/durability semantics, product-specific Worker/DO/Queue adapters, explicit model-level budgets for dynamic call frames, and stronger independent scheduler/capability oracles. The current profile assumes fault-free direct calls, no entire-handler lock, and no persistence guarantee. A cyclic call graph may exhaust the 64-total-frame bound and return `INCONCLUSIVE`; a call is not a Queue delivery. These are intentional omissions printed in CLI/JSON assumptions, not completion of the RFD.

Pickup commands:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run -- check examples/actor-keyed.fml
cargo run -- check examples/actor-interleaving.fml --trace-out /tmp/interleaving.trace.json # expected exit 1
cargo run -- replay examples/actor-interleaving.fml /tmp/interleaving.trace.json
```

## What works

- Top-level `let name = (typed, parameters): ReturnType { body }` functions; the last expression is the result. Pure functions can call other pure functions, including through a pattern match. Cycles are rejected by an acyclic dependency graph.
- `stateless actor Name { handler = function }` accepts a single-argument function. Each input has its own invocation frame. See [`examples/actor-stateless.fml`](../../examples/actor-stateless.fml).
- `stateful actor Name { state: Type = initial; handler = function }` accepts functions with `(owner: Actor<Type>, message: Input)`. Only the actor handler receives this state capability. `owner.state` reads and `owner.set(value)` writes the actor's owned state; the example demonstrates persistence between requests and a checked failure. See [`examples/actor-counter.fml`](../../examples/actor-counter.fml).
- The actor core works without any D1/Cloudflare declaration; existing D1 effect boundaries, invariants, temporal properties, explicit weak fairness, and validated replay remain operational. The profile ID for new models is `actors-v0`; old `cf-core-v0` example models continue to work via a transitional `worker` parser shim that converts handler bodies and `respond(value)` into functions and stateless actors. New functions/actors with the old profile are rejected. Replay artifacts now use format version 2 because actor state and function frames change their structure; old artifacts are refused rather than reinterpreted.
- The checker still uses `requests(Actor.method)` and `once Actor.method(value)`, so the example traces exercise actual actor entrypoints, not arbitrary Rust fixtures. Tests in `tests/actors.rs` exercise both actor types and error cases.

## Important limitations / decisions before merging

- **No actor-to-actor calls yet.** `call(Actor.method, message)` is rejected. Binding references may participate in cycle detection, but there is no scheduler/RPC implementation. Do not change the error to silent acceptance.
- **Exactly one static instance per stateful actor name.** There are no keyed instances, durability, crash recovery, eviction, queues, timeouts, or resource-specific actor backends. The state is retained only as a modeled value. `Actor<State>` is an owned capability, not a serializable or forgeable message. A singleton is insufficient to model Durable Objects; keyed identity and location must precede any Cloudflare mapping.
- `owner.set(...)` currently runs inside a deterministic local segment and does not by itself open an interleaving boundary. An external D1 operation suspends the function; other input slots can run. Clarify how state ownership, local atomicity, snapshot reads, and external calls compose before promising DO parity.
- Inference is conservative but not a general effect system: the current implementation walks expression trees in `src/functions.rs`, rejects recursive functions and mixed specification-inspection/I/O, and permits pure function calls through `src/semantics.rs`. Review lexical scoping, shadowing, declaration ordering, and capability non-escape carefully before broadening the syntax.
- Typechecking presently treats ordinary functions and handler functions similarly, with one supported data parameter for stateless actors and two parameters for stateful actors. No anonymous lambdas, closure capture, actor construction from a function value, modules, or user-defined generic types. Effects inside nested pure expression calls should continue to be rejected, never silently evaluated as atomic work.
- The old `worker` syntax is a **compatibility shim for regression tests**; decide whether to retain it as a Cloudflare frontend, migrate older fixtures, or remove it after the new actor vocabulary stabilizes. Do not silently change `cf-core-v0` semantics or interpret pre-spike traces with the new profile.
- Current RFD/README describe the previous Worker/D1 baseline. Treat this note and the new examples as the spike's truth. Write a proposal RFD for the core abstraction and backend semantic packages before declaring a stable public contract.

## Where to continue

1. Run `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked` and exercise both new examples via `cargo run -- check examples/actor-*.fml`. Buggy `actor-counter.fml` exits 1 by design. Replay its trace with `--trace-out /tmp/actor.trace.json` and `fml replay examples/actor-counter.fml /tmp/actor.trace.json`.
2. Design a typed actor address and call protocol: keyed instance identity; suspended caller continuation; message/reply serialization; concurrency within one keyed actor vs across actors; fairness and failure choices. Add safety and liveness fixtures **before** claiming RPC support.
3. Split a general model core (functions, actors, owned state, actions, properties) from optional semantics packages (Cloudflare Worker/DO/Queues/D1). Today D1 remains a built-in, while the new actors are generic. Preserve distinct resource semantics.
4. Move pure evaluation away from recursively reinterpreting `Stmt` blocks if expression or program size grows. The current recursion is guarded by source nesting and acyclic function dependency/cost checks but should gain fuzz coverage for deeply nested pure calls, and source spans for nested calls should identify the actual call site.
5. Add tests comparing old and new stateless examples, actor-local atomicity vs D1 suspension, effect rejection in pure property expressions, capability escape attempts through constructors/aliases/collections, and old-format replay refusal. Resolve any semantic drift by versioning the profile.

No merge, deployment, or Cloudflare guarantees are implied by this spike.
