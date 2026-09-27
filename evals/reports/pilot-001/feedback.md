# Pilot 001: findings and quality review

## Result

All **18 sequential configurations actually ran**, using local Pi 0.87.1 and
`openai-codex/gpt-6-luna`, high thinking. Each started a fresh project with only a
specification; assisted runs additionally received the checker and generic tool
documentation. No implementation or application tests were supplied.

| Measure | Baseline | FlareML-assisted |
|---|---:|---:|
| Implementations passing every fixed scenario | 8/9 | 8/9 |
| Scenarios passed | 168/192 | 168/192 |
| Normal agent completions | 9/9 | 6/9 |
| Trials passing both execution and scoring | 8/9 | 6/9 |
| Agent wall time, total | 27.0 min | 46.6 min |
| Reported tokens, including cache | 1,068,951 | 4,585,772 |
| Reported uncached input tokens | 213,310 | 407,129 |
| Reported output tokens | 65,625 | 111,827 |
| Token-budget stops | 0 | 3 |

In this pilot the assisted workflow used **1.72× agent time** and **4.29× reported
aggregate tokens**, with no improvement in the number of passing implementations.
The uncached-input ratio was 1.91× and output-token ratio 1.70×; the headline total
includes repeated cache reads, not just newly generated text. Three interrupted
runs have conservatively incomplete usage records. Prices in the machine-readable
report are catalog/provider estimates, not invoices.

This is **one sample per task/language/mode**, not evidence of a general causal
effect. Both modes largely chose a single aggregate CAS document, making the
core implementation fairly approachable for this model. Do not conclude either
that FlareML cannot help, or that merely adding it improves generated systems.

## Actual failures, not discarded trials

- **Baseline Go payments:** completed, compiled, but emitted `null` instead of
  `[]` for initialization. All 24 scenarios stopped at this protocol defect.
  [Independent one-line reproduction](payment-go-baseline-failure.md).
- **Assisted Go payments:** compiled but deleted the original pending request
  after initializing storage, then returned an invalid `conflict`. All 24
  scenarios failed. It also exhausted the token budget and omitted `DESIGN.md`.
  [Independent reproduction and model gap](payment-go-failure.md).
- **Assisted Go shortener and TypeScript payments:** generated code passed all
  fixed scenarios, but development exceeded the common token budget. These
  remain failed executions, not promoted to passing trials.

The 48 failed scenarios are not 48 distinct defects. The two Go payment failures
have different causes, and the baseline failure prevents assessing its ledger
behavior through this unmodified protocol. No candidate was repaired or replaced
for the reported scores.

## What the models actually establish

All nine candidate-authored final models independently recheck as complete
`VERIFIED_IN_SCOPE`, with reached cover claims. Reruns exclude the documentation
demo and copied `.fml/runs/model.fml` files. See [model-audit.json](model-audit.json)
for source hashes, properties, graph sizes, and results.

| Assisted task/language | States | Useful scope / important omission |
|---|---:|---|
| URL / Rust | 21 | Durable ownership agrees with acknowledgements for one code; business operation is atomic, no read/CAS retry or idempotency model. |
| URL / Go | 21 | Ownership/decision agreement; also contains a type-membership invariant mislabeled as immutability. No actual CAS loop or key binding model. |
| URL / TypeScript | 48 | Separate read/CAS steps, stale-snapshot contention, and successful-owner agreement. No application-key retries, restart, or multiple candidates. |
| Inventory / Rust | 52 | Explicit confirm/cancel CAS race and terminal-state stability. Stock accounting, initialization, and general reservation maps omitted. |
| Inventory / Go | 201 | One-unit conservation and two-ID reservation lifecycle, with progress. Application transitions are atomic; CAS collisions/restarts omitted. |
| Inventory / TypeScript | 21 | Two clients and one atomic stock-taking operation; no reservation lifecycle, initialization, CAS retries, or recovery. |
| Payments / Rust | 81 | Small charge/retry abstraction; CAS outcome is nondeterministic instead of checking the expected document. Does not verify the full ledger/refund protocol. |
| Payments / Go | 81 | Both operation labels access the same effective payment field; CAS ignores expected equality. Initialization, refunds, and implementation request-context lifetime omitted. |
| Payments / TypeScript | 51 | Actual read/CAS retries and bounded completion for two same-key charges. Safety is enum membership; refund code is not exercised by startup, and there is no monetary conservation property. |

A verified model and a passing workflow smoke gate are **not interchangeable with
model adequacy or implementation correctness**. Some models usefully exercise
interleavings, but others assume much of the serialization behavior that needs
justification. Several claimed safety properties cannot detect important task
failures. The payment initialization bug is a concrete model-to-code gap.

The saved checker history contains **31 INVALID_MODEL reports and 16 complete
verified reports**, with no saved violation counterexample. Observed errors
include statement terminators, reserved identifiers, and unavailable implicit
`self` / initializer variables. This pilot shows substantial onboarding friction;
it does not show a saved service-level counterexample driving a code correction.

## Harness confidence and limitations

- The trusted CAS worker passes all 64 frozen task/seed scenarios. Mutation
  controls detect broken idempotency, ignored CAS failures, early acknowledgement,
  overselling, and repeated refunds. Known-answer and real-time-order tests pass.
- The sequential oracle does not use FlareML verdicts or candidate tests. Its
  trusted self-test worker shares transition code, so these controls are useful
  but not a substitute for independent review of the oracle.
- Initial invocations within a batch are concurrent; real-time ordering is
  enforced across batches and by the oracle's interval constraint. This is not
  exhaustive coverage of all arrival histories or deployed network failures.
- The suite measures functional service cores, not HTTP integrations, throughput,
  memory use, or full production reliability. TypeScript is transpiled by Bun;
  candidate static typechecking is not an independent scoring requirement.
- The largest stored JSON value in the recorded histories was only **193 UTF-8
  bytes**. The suite does not establish correctness at advertised size bounds.
- Workflow counters are shell-keyword heuristics, not execution tracing. The
  independent model reruns and this manual review supplement that weak gate.
- Local directories are not an adversarial sandbox. The campaign is suitable for
  trusted agent pilots, not claims of leakage-proof hidden tests.
- All failures, partial projects, original Pi streams, and scheduler histories
  remain retained. No result was silently retried or selectively excluded.

## Changes after the frozen campaign

Original evaluator source and hashes remain under
`_evalkit-campaigns/pilot-001/harness/`. After the campaign, the live judge was
hardened to enforce storage size using **UTF-8 bytes**, rather than JavaScript
code units, and reject a missing stored value. Current definitions identify this
as `judgeRevision: 2`. The original scores are unchanged; every actual pilot
stored value was at most 193 bytes, so the corrected threshold cannot change
these recorded cases. A regression test covers multibyte overflow.

Harness typechecking and test discovery now explicitly exclude captured candidate
projects and frozen source snapshots. Future campaigns automatically snapshot
the harness and refuse to overwrite an existing snapshot. These maintenance
changes did not alter the prompts, budgets, schedules, or loaded graders during
the 18-run campaign.

## Recommended next experiment

1. Improve the concise tool guide: explicit self/reply addresses, initializer
   scope, reserved words, and the distinction between an atomic application
   operation and a read/CAS/retry implementation. Do not supply task solutions.
2. Define an independently reviewed model-adequacy rubric: actual CAS semantics,
   non-tautological conservation/idempotency properties, representable bad states,
   initialization and recovery, and a documented refinement gap to code.
3. Require a real stdout-protocol smoke test. Ordinary in-memory tests can miss
   Go's nil-slice encoding, while a concurrency model will not catch it.
4. Expand a separately versioned judge with empty-state reads, overlapping
   updates/inspection, more workers, larger histories, and UTF-8/size boundaries.
   Choose any multi-key or storage restrictions for realistic requirements,
   not merely to make one favored condition win.
5. Repeat matched trials with a preregistered order/seed plan. A separate paired
   experiment can compare alternative time/token budgets or tighter modeling
   budgets. Do not relax only the failed assisted cells and pool the retries
   into this pilot.

[Full per-cell metrics](summary.md) · [Machine-readable data](summary.json)
