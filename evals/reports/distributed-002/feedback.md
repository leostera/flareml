# Distributed-002: completed fresh-agent comparison

## Result

All 24 task/language/mode cells now have full agent-generation attempts against
the corrected frozen suite. Four provider-transport failures were retried from
scratch, giving **28 recorded attempts**. The original failed attempts are kept;
no coding failure or token-budget stop was retried. There were no artifact-regrade
agents in this comparison. No candidate implementation was repaired.

| Mode | Checkpoint implementations passing | Manifest implementations passing | Total passing | Normal agent completions | Token-budget stops |
|---|---:|---:|---:|---:|---:|
| Baseline | 4/4 | 4/4 | **8/8** | 8/8 | 0 |
| FlareML | 3/4 | 4/4 | **7/8** | 8/8 | 0 |
| TLA+ | 2/4 | 4/4 | **6/8** | 4/8 | 4 |

Each checkpoint implementation faced 84 schedules; each manifest implementation
faced 12. Passing is bounded test success, not proof. Workflow smoke gates passed
for all 24 selected attempts, but do not establish model adequacy.

## Failures

- **Checkpoint / Rust / FlareML:** 72/84, overlapping checkpoints after a restarted
  primary acts on an expired replay prefix without knowing the live log tail.
- **Checkpoint / Rust / TLA+:** 72/84, the same class of replay/overlap failure;
  additionally stopped by the token budget.
- **Checkpoint / Go / TLA+:** 0/84, invalid wire output. The implementation returns
  a nil `[]Action` from initialization; `json.Marshal` emits `null`, not the
  required `[]`. Also stopped by the token budget.
- **Checkpoint / TypeScript and Python / TLA+:** code passed 84/84, but both agents
  hit the token budget. Their code correctness is not a normal execution success.

The twelve failed schedules in each Rust result repeat one defect across seeds
and node-ID permutations; they are not twelve distinct bugs.

## Costs (selected attempts)

Modeling, checking, implementation and candidate testing share the same budget:
12 minutes, 80 assistant responses, 600,000 provider-reported total tokens
including cache. Limits are checked at response boundaries and can overshoot.

| Mode | Agent seconds | Tokens including cache | Time / baseline | Tokens / baseline |
|---|---:|---:|---:|---:|
| Baseline | 1,912.5 | 1,189,567 | 1.00× | 1.00× |
| FlareML | 2,225.0 | 2,818,153 | 1.16× | 2.37× |
| TLA+ | 2,506.8 | 3,536,283 | 1.31× | 2.97× |

Cache tokens are not equivalent to uncached input or output cost. Relative to
baseline, FlareML used 1.72× uncached input / 1.16× output; TLA+ used 2.03× /
1.23×. Prices in the JSON are provider-configured estimates, not invoices.
Interrupted/provider-failed requests may have unreported usage.

Including the failed network attempts: baseline 1,926.8 seconds / 1,189,567
reported tokens; FlareML 2,239.4 / 2,818,153; TLA+ 2,649.3 / 3,541,850.

## What the models establish—and do not

The selected FlareML histories contain **18 VERIFIED_IN_SCOPE and 19
INVALID_MODEL reports, with no saved VIOLATED report**. There is therefore no
saved FlareML counterexample-driven design repair demonstrated in this campaign.
This does not mean reasoning about a model had no effect.

The failing Rust/FlareML model has one global protocol actor and a predetermined
FIFO grant/completion sequence. Its 13-state graph checks primary-versus-holder
identity and reaches repeated completions, but omits crashes, replay, per-node
lag, and separate log commit/delivery. The implementation's failure is outside
that model. DESIGN.md admits the missing recovery guarantee rather than solving
it using the stated finite lease bound.

The failing Rust/TLA+ model also omits restart, and bounds the log to **one append**.
Its verified safety check cannot establish safety of multi-entry recovery. These
are concrete examples of green model checks not justifying generated code.

TLA+ histories contain 14 verified checks, 19 invariant violations, 2 incomplete
searches and 33 other nonverified invocations. **Those 19 violations are not a
bug-discovery count**: they include deliberately negated reachability targets and
require model/configuration/trace review to distinguish encoding errors from
protocol repairs. No aggregate TLA+ design-discovery claim is made here.

## Retry handling and evidence

The final four original cells encountered `WebSocket error` / `fetch failed`.
Authentication was ready, connectivity returned, and all 42 frozen harness file
hashes matched before starting the retries. Retry order preserved the remaining
cells: TypeScript/TLA+, then Python/FlareML, Python/TLA+, Python/baseline. All four
fresh attempts completed normally and passed 12/12.

- `comparison.json`: selected 24 cells, all 28 attempts, costs and provenance.
- `retry-plan.json`: explicit transport-failure selection, not outcome shopping.
- `summary.{json,md}`: untouched original-campaign result accounting, including
  the four transport failures; these are not the consolidated comparison.
- `../distributed-002-retry-typescript/` and `../distributed-002-retry-python/`:
  separate native retry campaign summaries and checker histories.
- `formal-history.json`: saved checker statuses/configuration evidence.
- Private EvalKit reports, full Pi streams, candidate projects, scheduling traces
  and frozen harness snapshots remain under the ignored `_evalkit-*` directories.

## Interpretation

In this bounded pilot, neither assisted workflow improved implementation
correctness over baseline, and both consumed more tokens. All manifest cells
passed, so that task still has a ceiling effect. The checkpoint failures show
that richer tasks alone do not ensure adequate agent-authored models.

This is **one generation per cell**, not a statistically supported causal ranking
of languages or tools. TLA+ results are especially budget-sensitive: every
checkpoint TLA+ run hit the cache-inclusive token cap. The experiment compares
agent + documentation + tool workflows, not intrinsic formal-language power.
Shared logical time, supplied consensus, local-process scheduling, public-source
familiarity and lack of an adversarial sandbox remain important limitations.
