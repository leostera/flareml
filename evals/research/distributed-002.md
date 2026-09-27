# Distributed-002: fresh agent evaluations, corrected frozen judges

This replaces the stopped `distributed-001` diagnostic campaign. All 24 cells
run the real Pi coding agent again, in new workspaces with SPEC.md only (plus the
assigned checker's generic tools/docs). No saved candidate is supplied, resumed,
or repaired; no frozen-artifact adapter participates. Existing reports remain
historical evidence, not rows in this comparison.

## Matrix and budgets

Two tasks × Rust/Go/TypeScript/Python × baseline/FlareML/TLA+; one generation per
cell, sequential and rotating treatment order. Model remains
`openai-codex/gpt-6-luna`, high thinking. Each generation gets 12 minutes,
80 assistant responses, and 600,000 reported total tokens including cache.
Modeling, checking, implementation and candidate-written tests share that budget.

## Frozen scoring (judge revision 2, spec revision 2)

- **Checkpoint: 84 schedules.** Seven prefixes × four seeds × three node-ID
  permutations. Prefixes cover handover/replay, lag/stale results, primary
  restart, partial primary replay after two and three committed entries,
  completed-lease replay, and promotion while actively checkpointing. Every
  prefix is followed by a healthy progress suffix. All three physical nodes
  are exercised as stable primary.
- **Manifest: 12 schedules.** Uncertain publication, in-flight effects surviving
  frontend restart, and a deliberately held read across publication/collection,
  each under four seeds. The duplicate workload group is removed.

The checkpoint specification explicitly clarifies that same-primary recovery is
not a caught-up promotion, input examples do not fix the primary's ID, and work
completing exactly at its deadline counts as completed. The healthy progress
suffix delivers pending messages promptly; arbitrary fair-but-unbounded network
latency is not asserted compatible with a 32-tick deadline. Output arrays have an
explicit 64-action resource bound. These clarifications are identical across
modes and languages; no solution algorithm or upstream model is supplied.

Preflight: the independent private reference passes all 84 checkpoint and all
12 manifest schedules. A two-unit-lease reference also passes. Mutants detect
premature lease preemption, absent progress, hard-coded holders, both partial-
replay holder policies, missing durable completion hints, omitted promotion
abort, stale-read-as-missing, unsafe cleanup, regression and missing collection.
The complete harness has 10 passing tests / 355 assertions at launch preparation.

## Evidence and changes

EvalKit performs actual agent trials, stores trajectories and candidate projects,
and applies the frozen judges directly. A complete source snapshot and hashes
are captured before the first generation. Correctness, workflow smoke gate,
execution completion, model adequacy, and costs remain separate.

If a material judge defect is discovered, stop/version the suite and redo the
agent evaluations—not a substitute comparison made from empty-transcript
artifact-regrading trials. Preserve failed and interrupted runs as diagnostics.
A STOP file in the campaign directory, or SIGINT/SIGTERM to the campaign runner,
requests a stop after the current trial completes.

These remain bounded, trusted-local-process experiments. Shared logical time and
runtime deadlines simplify the source system; consensus is provided. Passing
is not proof, one generation per cell is not a causal estimate, and public-source
training contamination is possible. See `public-specs.md` for pinned sources and
licenses, and `distributed-v2.md` for the historical design/review process.
