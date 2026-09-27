# FlareML-assisted coding evaluations

EvalKit evaluates **Pi writing projects from scratch** with
`openai-codex/gpt-6-luna` (high thinking). There is no candidate implementation,
build manifest, entrypoint, test suite, or solution fixture. Each candidate starts
with only `SPEC.md`. Assisted conditions additionally get their checker and
generic language documentation, never an upstream task solution.

## Recorded pilot

[`reports/pilot-001/feedback.md`](reports/pilot-001/feedback.md) reviews the completed
18-cell campaign, actual failures, model adequacy, and next steps. Both modes had
8/9 implementations pass the fixed scenarios; assisted runs used 1.72× agent time
and 4.29× reported tokens, with three token-budget stops. This is descriptive pilot
evidence, not a general causal conclusion.

## Next-suite source research

[`research/public-specs.md`](research/public-specs.md) catalogs real public TLA+
specifications, neutral task extraction, and the proposed baseline/FlareML/TLA+
comparison across Rust, Go, TypeScript, and Python. It includes actual TLC
reproductions of known storage-cleanup and distributed checkpoint-coordination
bugs. Those source diagnostics are **not agent trials**. The expanded modes,
Python target, and two adapted distributed tasks are now implemented; see
[`research/distributed-002.md`](research/distributed-002.md) for the corrected
frozen suite and fresh 24-agent-run campaign. `distributed-001` was stopped and
retained as diagnostic evidence; its artifact reviews are not new agent runs.

## Suite

Twenty evaluations: each task exports Rust, Go, TypeScript/Bun and Python targets.

| Task | Rust | Go | TypeScript/Bun |
|---|---|---|---|
| URL shortener | `url-shortener-rust` | `url-shortener-go` | `url-shortener-typescript` |
| Inventory reservations | `inventory-rust` | `inventory-go` | `inventory-typescript` |
| Atomic payment ledger | `payments-rust` | `payments-go` | `payments-typescript` |

The original task IDs also have `-python` targets. Added task families are
`checkpoint-coordination` and `manifest-publication`, with the same four suffixes.

Each runs in `baseline`, `flareml` and `tla` modes: **60 discoverable cells**.
The new-task campaign selects 24; the original 18 recorded results are unchanged.
All modes have the
same functional spec, language requirement, independent judge, model, thinking
level, and total budget. Baseline uses ordinary development tools. Assisted Pi
must model/check/refine the design before implementing, save models and checker
evidence, and explain the mapping to code. EvalKit invokes Pi; Pi decides and
performs the actual coding and checker calls. The evaluator never writes the
solution for Pi.

The original three tasks exercise distributed **service cores**, not HTTP routing or deployed
cloud infrastructure. Two separate frontend processes operate through a supplied
linearizable KV store using asynchronous reads and single-key compare-and-swap.
The candidate chooses its own internal data representation, including a single
aggregate document if appropriate. Private schedules interleave client requests,
KV execution, and storage completions; retry lost acknowledgements; and restart
frontends while preserving committed storage. Payments are internal atomic ledger
changes, not a claim to exactly-once external card charges.

## Run

Requirements: Bun 1.4.2, local Pi 0.87.1 or compatible JSON CLI, authenticated
`openai-codex/gpt-6-luna`, stable Rust, Go, Python 3, Java, and locally cached
`serde_json 1.0.151`.
The recorded campaign captures actual tool versions and binary SHA-256.

```sh
# Repository root; builds the exact checker supplied to assisted Pi.
cargo build --release --locked
cd evals
bun install --frozen-lockfile
bun run check
bun scripts/setup-tla.ts        # checksum-pinned TLC; no provider calls
bun run test                    # provider-free oracle / mutation tests
bun run matrix:plan             # 60 cells, no model calls
pi auth check --provider openai-codex --json  # never print credentials

# These invoke the real model and can incur costs.
bun run campaign --id=my-campaign --suite=distributed
# Or select a paired original control:
bun run campaign --id=one-pair --only=url-shortener-rust --modes=baseline,flareml
# Standard EvalKit CLI (also sequential):
bun run evals
bun run evalkit run-matrix flareml-impact --eval inventory-go --mode flareml --local

bun run summarize my-campaign
bun scripts/audit-models.ts my-campaign  # independently rerun authored FlareML models
bun scripts/audit-formal.ts my-campaign  # saved FlareML + TLC histories
bun scripts/progress.ts --active
bun run dashboard
```

`campaign` uses EvalKit's Effect-based `runEval`, not a replacement execution
framework. It runs one cell at a time and rotates treatment order across
task/language groups. Select `--suite=simple|distributed|all`, `--languages=...`,
and `--modes=...` to restrict costs. The CLI matrix discovers all 60 cells;
`--dry-run`'s empty `evals` selection means no ID filter, **not zero discovered
evals**. Campaign errors remain recorded; failed cells are not silently
replaced by retries.

Default budgets: 12 minutes, 80 completed assistant responses, and 600,000
provider-reported total tokens (including cache tokens). The adapter enforces
limits; EvalKit currently only forwards budget parameters. Token limits are
checked on received events, so the current provider response can overshoot a
limit. Interrupted usage is marked incomplete. The subprocess receives TERM then
KILL on deadline. Pi processes use a new session, explicit tools/model, no
inherited project context/skills/extensions, and the common system prompt.

## Scoring

1. **Independent correctness:** compile and run the candidate against 20 URL,
   20 inventory, or 24 payment scenarios. Each scenario is a hand-authored semantic
   case under one of four frozen scheduler seeds. An independent sequential
   specification exhaustively searches batch serializations constrained by
   request/reply real-time order. Both replicas' snapshots, retry outcomes,
   conflicts, balances, and durable state are checked. No score depends on the
   candidate passing its own tests or on a FlareML verdict. Rust and Go are
   compiled; TypeScript is Bun-transpiled, not independently typechecked. Python
   is syntax-compiled then run with `python3 -u main.py`. The distributed tasks
   use separate safety/progress judges, with explicit log/storage boundaries.
2. **Workflow smoke gate:** baseline must have no shell commands mentioning
   FlareML or TLC; FlareML-assisted runs must have such commands, saved model files, and a complete
   verified report. These keyword counters are heuristic, not OS-level exec
   tracing (reading a `.fml` file can increment them). This is not a proof of model
   adequacy. TLA+ uses recorded wrapper invocations and completed invariant-check
   receipts; this still does not establish property adequacy. Read saved models and design
   notes separately; a trivial model must not be treated as evidence for code.

The report retains correctness and workflow **separately**. EvalKit's aggregate
score averages the two rules; use the correctness column rather than that mean
when comparing code quality. Build/protocol/nontermination failures count as
failures. Partial candidates can still be scored after an agent error, but a
failed execution does not become a passing trial.

`bun run test` checks known oracle answers, real-time constraints, all schedules
against a trusted reference worker, and mutation controls (forgotten idempotency,
ignored CAS failures, acknowledgement before durable commit, overselling, and
double refund). The trusted worker exists only under `tests/`; it is never copied
to a candidate workspace. Its state transition logic shares the sequential
oracle, so mutation controls and known-answer tests supplement—not replace—an
independent external audit of the judge.

## Evidence and feedback

- `_evalkit-results/`: native EvalKit manifests, trajectories, scores, candidate
  snapshots, and failure evidence; usable in the EvalKit dashboard.
- `_evalkit-evidence/<trial-id>/`: original Pi JSONL, stderr, exact prompt,
  measured usage/timing, build log, and complete private scheduling histories.
- `_evalkit-campaigns/<id>/`: planned cells, completed runs, environment and checker
  identity, plus an automatic evaluator-source snapshot with content hashes.
  `bun scripts/snapshot-harness.ts <id>` is available for a campaign lacking a
  snapshot; it refuses to overwrite an existing one.
- `reports/<id>/summary.{json,md}`: compact shareable results and paired deltas.
- Candidate/evaluator workspaces persist under the OS temporary directory's
  `flareml-evalkit-workspaces/`. Tool binaries and ordinary dependency/build
  directories are removed from the candidate before report snapshotting; project
  source, tests, design notes, and checker evidence remain.

Raw streams may include reasoning, local paths, source, and provider metadata.
They are ignored by Git. Review before sharing. Authentication files are never
copied or printed. These are **trusted local-agent pilots**, not hardened hostile
code execution: separate directories and instructions are not a security sandbox.
The agent has shell access for toolchains. Prompt adherence, access to sibling
workspaces, and covert alternative model/checker use require audit; do not claim
cryptographic isolation or leakage-proof hidden tests.

One trial per cell is a harness/quality pilot, not a statistically supported causal
claim. Account for cache reads, toolchain warmup, treatment order, infrastructure
failures, and modeling time. Prices are provider-reported estimates, not invoices;
missing/zero catalog pricing is unknown, not free. Faster/smaller responses that
fail correctness or skip the assigned workflow are not improvements. FlareML's
finite abstraction must still correspond to the implementation. If both modes
pass everything, report a ceiling effect and add a separately versioned harder
benchmark rather than changing tests until a favored condition wins.
