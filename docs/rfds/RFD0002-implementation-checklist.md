# RFD0002 — implementation and acceptance checklist

Authoritative contract: [RFD0002 — Functions and actors as the modeling core](RFD0002-functions-and-actors.md). Branch: `spike/actor-generalization`.

**Status:** the generic, fault-free actor core is implemented with tests. This is **not full RFD/release acceptance**: the separate product-adapter gate and coverage-instrumented fuzz campaign below remain open. Do not substitute generic mailbox behavior for Cloudflare contracts.

## Generic language and scheduler

- [x] One `actor` form; singleton and finite keyed identities; stateful `init` plus state-in/state-out `handle_message`; stateless one-argument callbacks.
- [x] Typed transferable addresses, exhaustive message matching, transitive pure/send/inspector restrictions, no escaping owner capabilities.
- [x] Pure identity-only initialization after address enumeration; no configuration, send, or resource effects in `init`.
- [x] FIFO mailboxes per typed address; nondeterministic sender/target scheduling; separate optional input submission and atomic callback commit.
- [x] Ordered staged sends, including nested helpers and self-sends. No reply can be processed before its sender commits.
- [x] Required per-address `mailbox_bound`; explicit cutoff rather than dropping, disabling, or partly committing an overflowing send.

Evidence: [`tests/messaging.rs`](../../tests/messaging.rs), [`tests/actor_hardening.rs`](../../tests/actor_hardening.rs), [`examples/actor-messages.fml`](../../examples/actor-messages.fml).

## Properties, identity, fairness and evidence

- [x] `inputs(Actor)` includes all declared external slots; typed payload/target plus monotone submission/processing flags.
- [x] `messages(Actor)` includes all potential lifetime slots, even before generation; explicit `message_bound` per actor declaration, never reused. Covers expose reached antecedents; unused slots are not mistaken for an empty initial collection.
- [x] Identical payloads remain separate messages/inputs. Processing an input is distinct from processing its follow-up/reply.
- [x] Fair/unfair reply progress, optional-input starvation, a busy self-sender versus another actor, and a missing-reply failure under fairness.
- [x] Independent FIFO reference machine compares all successor labels, fairness, queue contents, external observations and cutoffs for two forwarding actors across workloads and capacities.
- [x] Existing independent recurrent-edge temporal oracle remains passing.
- [x] Format-5 v2 trace serialization, metadata/bound/provenance checking, source-mapped sends, finite and lasso replay. Old v2 format 4 is rejected; old profiles keep format 3.
- [x] Public CLI safety/liveness/JSON/replay tests, colored output, `NO_COLOR`, and no ANSI escapes in JSON.

Evidence: [`tests/message_observations.rs`](../../tests/message_observations.rs), [`tests/messaging_oracle.rs`](../../tests/messaging_oracle.rs), [`tests/temporal_oracle.rs`](../../tests/temporal_oracle.rs), [`tests/cli.rs`](../../tests/cli.rs).

## Hardening and compatibility

- [x] Preserve the existing `cf-core-v0`, `actors-v0`, and `actors-v1` regression suite and meanings.
- [x] Keep the old `worker` frontend as compatibility syntax; reject it in v2. No automatic semantics-changing source migration.
- [x] Reject recursive v2 data until a depth-bound profile exists; bound parser trees (including Pratt left spines), function expansion/depth, runtime evaluation depth, value size and total addresses.
- [x] Domain validation catches intermediate values in pure helpers, not only committed state.
- [x] Source and trace fuzz targets build; trace target now validates decoded artifacts against the v2 fixture.
- [x] Local stable libFuzzer smoke runs: 1,000 source mutations and 1,000 trace mutations with the message fixture/trace as seeds. **No coverage/sanitizer instrumentation was available.**
- [ ] Coverage-instrumented nightly `cargo fuzz` runs with seeded valid and corrupted v2 source/traces; fix findings before stable release. The current environment has neither `cargo-fuzz` nor a rustup nightly toolchain installed.

## Product-adapter gate — not implemented

RFD0002 implementation step 5 remains open. These are **separate profiles**, not additional behaviors inferred from the generic actor syntax:

- [ ] Worker trigger/invocation/response and failure contracts, without inventing persistent Worker identity.
- [ ] Durable Object routing, volatile versus durable state, storage transactions, suspension/gates, restart/eviction behavior and litmus tests.
- [ ] Queue producer/consumer, delivery attempts, duplicates, acknowledgment, retry/exhaustion and litmus tests; no generic FIFO or exactly-once substitution.
- [ ] Explicitly split state commit and publication in adapters where atomic state-plus-send is not justified.
- [ ] Source/assumption ledger, profile registration and replay compatibility for each adapter.

Fault/retry/restart, synchronous RPC convenience, arbitrary check-supplied initialization configuration and production conformance are not implemented by `actors-v2`. This checklist does not relabel them as generic-core guarantees.

## Reproduce the generic acceptance checks

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run -- check examples/actor-messages.fml --trace-out /tmp/messages.trace.json
cargo run -- replay examples/actor-messages.fml /tmp/messages.trace.json
```

With a nightly toolchain and `cargo-fuzz` installed, use the existing `source` and `trace_json` targets. Seed `fuzz/corpus/source/` from the examples and `fuzz/corpus/trace_json/` from generated format-5 artifacts. Corpus and artifacts are intentionally gitignored; preserve regression findings as small checked-in tests.
