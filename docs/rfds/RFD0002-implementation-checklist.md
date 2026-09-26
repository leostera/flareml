# RFD0002 — implementation and acceptance

Contract: [RFD0002](RFD0002-functions-and-actors.md). Development is on `main`.

## Implemented

### One language, one engine

- [x] One actor form, pure singleton/keyed initialization, state-in/state-out handlers and one-way typed sends.
- [x] No `semantics` selector, legacy parser/profile dispatch, Worker/D1 primitives, owner capabilities, synchronous calls, continuation frames, or old trace readers.
- [x] Shared local statement evaluator for pure functions and message turns; only turns receive an outbox.
- [x] Stable Rust toolchain and normal CI; clap-derived CLI.
- [x] Required semicolons for bindings and intermediate statements; unterminated tail values, terminated-tail `unit`, explicit match-arm separators, and no whitespace-only sequencing. Manually migrated examples/fixtures; regressions cover parenthesized tails after sends and Result-discard rejection.
- [x] Current-only examples for sequential computation, eligibility policy, replies, missing replies, routed accounts, lost updates, and their atomic repair.

### Property surface

- [x] Only `property` declarations. Explicit `always`, supported temporal forms, and whole-body `reachable` over pure predicates.
- [x] Reject bare predicates and nested/mixed reachability. `exists` remains data quantification.
- [x] Whole-body safety is checked at initialization and as successor states are discovered, including before a later sibling hits a search limit.
- [x] Do not misclassify `always eventually` as safety; retain conjunction/stable-quantifier temporal checking.
- [x] Zero-step reachability, reached witnesses on incomplete graphs, unreachable only after closure, non-failing unreachable exit policy.
- [x] Property selection, source spans, current-format serialization, finite/lasso replay and tamper detection.
- [x] Count unreached response antecedents rather than labeling an entire message property vacuous because spare slots are unused.

### Execution and observations

- [x] Finite FIFO mailboxes, nondeterministic inter-address scheduling, optional external submissions, separate enqueue/processing transitions.
- [x] Atomic state/outbox commit; nested helper sends preserve order; self-send follows dequeue and sees committed state.
- [x] Capacity overflow produces inconclusive, never partial commit/drop/disabled-send semantics.
- [x] Stable input observations and bounded lifetime message identities, including generated and identical-payload messages. No slot recycling.
- [x] Fair/unfair progress, optional-input starvation, missing replies under fairness, and busy self-sender versus another mailbox.
- [x] Typed addresses and exhaustive branches; transitive inspection/send restrictions; closed non-recursive data.
- [x] Explicit host bounds for syntax, call/data depth, expansion, value size, identities, local evaluation work and temporal expansion (including empty inner domains).

## Independent validation

- [x] Existing mailbox reference machine compares every reachable edge, fairness flag, input status and queue identity, plus capacity cutoffs, over small forwarding workloads.
- [x] Exhaust all **729** three-state/two-message deterministic transition tables, comparing every reachable edge for distinct and identical input payloads.
- [x] Exhaust **1,024** two-state topology/fairness/predicate combinations across all seven temporal patterns (**7,168** obligation checks).
- [x] Generated three-state temporal graphs include shared action IDs, intermittent enablement and fair self-edges.
- [x] Compare temporal outcomes with an independent recurrent-edge-subset oracle; independently validate produced walks, original enabledness fairness, and failed formulas using fixed points.
- [x] Source/CLI/JSON, color/NO_COLOR, malformed inputs, source/trace mutation, current-format rejection, domain limits, atomic failure and replay regressions.
- [x] 160 end-to-end Boolean self-message cases checked against an independent orbit/cycle oracle (safety, reachability, eventuality, recurrence, stabilization; fair/unfair).
- [x] Actor-renaming/declaration-reordering metamorphic checks and concrete counterexamples under increased mailbox bounds.
- [x] Default source/configuration/report/all-witness bundles; snapshot replay, corruption rejection, safe filenames, incomplete/invalid reports, I/O failure and concurrent allocation tests.
- [x] Inventory reservation and payment idempotency bug/repair pairs, including non-vacuous completion queries.
- [x] RFD0003 choice extension: independent candidate-product checks, branch-local outbox/observation isolation, constrained transcript replay, fairness/cutoff regressions, and loss/duplication examples checked per obligation.

## Remaining validation before calling the core stable

- [x] Run and inspect initial coverage/sanitizer-instrumented source and replay campaigns. Both passed; no findings to minimize (details below).
- [ ] Review coverage gaps and run longer seeded campaigns, especially deeply nested valid source and nearly-valid traces; passing random invalid bytes is not enough.
- [ ] Independent implementation review against the execution/property contract. Finite oracles and replay agreement are not a proof of correctness.
- [ ] Establish measured practical state-space limits on larger protocols before introducing optimization claims.

Instrumented evidence: [GitHub Actions run 36258519843](https://github.com/leostera/flareml/actions/runs/36258519843), on core revision `f582520`, passed both nightly sanitizer targets with valid source/trace seeds and seed `12345`. Source: **791,126 executions in 121 seconds**; trace JSON: **3,018,940 executions in 121 seconds**. Logs were inspected; corpus artifacts were uploaded. These are short smoke campaigns, not a coverage-completeness claim, and predate the run-bundle additions. Longer campaigns and coverage-gap review remain open.

A later local nightly installation succeeded for [RFD0003](RFD0003-nondeterministic-choice-and-faulty-links.md#recorded-validation-and-measurements). Its choice/source and format-7 replay campaigns passed; see that RFD for counts, seeds, and measured branching cutoffs. Nightly remains optional, only for fuzzing.

## Deferred scope, not generic-core blockers

- [ ] Imports/namespaces, reusable definitions, explicit finite static instances and check-supplied initialization: separate design, including replay source identity.
- [ ] Resource/transport libraries or adapters: specify real consistency, scheduling, storage and failure behavior. No Worker/DO/Queue/D1 equivalence is claimed, and no profile switch is planned.
- [ ] Crashes/restarts, retries/timeouts, dynamic spawn, suspended callbacks, RPC conveniences and shared-memory models: not part of the present execution contract.
- [ ] Symmetry/partial-order/symbolic reductions: only after soundness design and differential validation.

## Reproduce

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked -- check examples/counter-replies.fml --trace-out /tmp/replies.json
cargo run --locked -- replay examples/counter-replies.fml /tmp/replies.json
cargo run --locked -- check examples/missing-reply.fml --trace-out /tmp/missing.json # expected exit 1
cargo run --locked -- replay examples/missing-reply.fml /tmp/missing.json
```

Nightly is needed only for optional instrumented fuzzing; instructions are in the root README. Normal checking, replay, tests, independent oracles, and Clippy use stable.
