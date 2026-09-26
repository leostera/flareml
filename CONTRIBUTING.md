# Contributing to FlareML

FlareML is a finite systems modeling language and native Rust checker. Read the [user guide](README.md) for the CLI and the [current language/execution contract](docs/rfds/RFD0002-functions-and-actors.md) before changing behavior. The [RFD0002 implementation checklist](docs/rfds/RFD0002-implementation-checklist.md) tracks completed evidence and outstanding validation; draft proposals are not accepted language contracts.

## Build and validate

Normal development and CI use **stable Rust** (`rust-toolchain.toml`). No nightly toolchain, JVM, or external checker is required for the application or its tests.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Run a passing and a failing model through the CLI and replay their evidence after any source-to-trace changes:

```sh
cargo run --locked -- check examples/counter-replies.fml --trace-out /tmp/replies.json
cargo run --locked -- replay examples/counter-replies.fml /tmp/replies.json
cargo run --locked -- check examples/missing-reply.fml --trace-out /tmp/missing.json # expected exit 1
cargo run --locked -- replay examples/missing-reply.fml /tmp/missing.json
```

Each `check` also creates a unique `.fml/runs/<run-id>/` bundle with a source snapshot, configuration, report, and all available witnesses. It is ignored by Git. For test runs, use temporary directories or `--artifacts-dir` where appropriate; do not commit generated bundles or traces as evidence of general correctness. Check the report's `complete` and `cutoff` fields rather than treating a partial search as verification.

## Changing models, semantics, or diagnostics

- Preserve finite-scope honesty: a limit or capacity cutoff must not silently discard a transition or prove a claim. Keep safety, reachability, temporal fairness, deterministic `check.main` setup, optional input submission, and bounded runtime creation distinct. Every actor definition needs a `spawn_bound`, even when unused; definitions do not create instances.
- Preserve source-mapped diagnostics and replay that validates the source, check, action labels, snapshots, loop closure, and fairness. Version serialized artifacts when their representation or meaning changes; do not silently reinterpret an old trace.
- An actor is a modeled participant, not a deployment type. The core does not inject failures, retries, or product-specific durability. An explicit `choose` in a link actor can represent drop/duplicate outcomes, but weak mailbox fairness does not select favorable alternatives. Do not claim that a passing model proves production conformance. See [RFD0003](docs/rfds/RFD0003-nondeterministic-choice-and-faulty-links.md).
- Add positive and negative tests for syntax/effects, bounds, CLI exit/JSON behavior, and replay when changing the contract. Favor small independent reference models over reusing the implementation under test as the oracle.
- Update the language contract, README (user-facing behavior), and bundled manuals in `docs/skills/fml/` when public behavior changes. Statements now require explicit `;` unless they are a tail expression; choice is effectful and must remain a whole local binding initializer. `fml skills` and its topic subcommands embed those Markdown pages at build time; the CLI tests compare binary output with the source pages. An installed binary needs rebuilding to pick up documentation edits.

Every `.fml` file in `examples/` must have an expected verdict in `tests/examples.rs`, and a row in [examples/README.md](examples/README.md). The example test checks each verdict and replays available witnesses; for repaired scenarios, test that completion is *reachable* instead of relying on vacuous safety. If an example models a real-world protocol, state its atomicity, scheduling, and failure omissions explicitly. Compare the inventory and payment bug/repair pairs: the payment repair assumes a single modeled atomic ledger-and-charge turn, not a transaction with an external payment API.

The suite includes source-to-CLI tests, atomicity, type/effect rejection, finite bounds, fair/unfair progress, missing replies, and trace corruption. Independent oracles cover:

- all two-state graph/predicate/fairness combinations for seven temporal patterns and generated three-state graphs with shared action identities;
- FIFO scheduler transitions and cutoffs;
- all 729 three-state/two-message deterministic transition tables;
- 160 source-to-verdict Boolean self-message cases against an independent orbit/cycle oracle, including optional input starvation and weak fairness;
- independent Cartesian enumeration for small choice lists, helper choices/outboxes, branch-local encounter counts, choice fairness, transcript tampering and replay;
- independent enumeration of 155 deterministic setup configurations (populations, initializer values, FIFO, captured inputs, cutoffs), plus all transitions of a tiny two-coordinator spawn machine across four creation bounds and both fairness settings; future-instance temporal and allocation-replay regressions.

Metamorphic regressions check actor renaming, declaration reordering, and persistence of concrete counterexamples under larger mailbox bounds. These checks increase confidence; they are **not** a proof of checker correctness. Search currently uses exact state equality without symmetry, partial-order, or symbolic reduction.

## Optional instrumented fuzzing

**Nightly is optional and only for coverage/sanitizer-instrumented fuzzing.** It is not an application dependency. If available:

```sh
cargo install cargo-fuzz --locked
rustup toolchain install nightly --profile minimal
mkdir -p fuzz/corpus/source fuzz/corpus/trace_json
cp examples/*.fml fuzz/corpus/source/
cargo run --locked -- check examples/counter-replies.fml --trace-out fuzz/corpus/trace_json/replies.json
cargo +nightly fuzz run source -- -max_total_time=120
cargo +nightly fuzz run trace_json -- -max_total_time=120
```

A separate optional scheduled/manual workflow (`.github/workflows/fuzz.yml`) runs instrumented campaigns and saves artifacts. Initial short campaigns are recorded in the [acceptance checklist](docs/rfds/RFD0002-implementation-checklist.md); longer seeded campaigns, coverage-gap review, minimized regression fixtures, independent contract review, and practical state-space measurements remain open. Do not present passing fuzz campaigns as coverage completeness or formal verification.
