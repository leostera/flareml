# Actor generalization — checkpoint / pickup notes

Branch: `spike/actor-generalization`, based on the initial `main` checkpoint `327bd44`. This is a **spike**, not a finished replacement for [RFD0001](../rfds/RFD0001-initial-language-and-model-checker.md). No deployable runtime is produced.

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
