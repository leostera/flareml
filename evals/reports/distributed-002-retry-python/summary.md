# FlareML-assisted coding pilot: distributed-002-retry-python

Status: **completed**. 3/3 cells recorded. Model: `openai-codex/gpt-6-luna` (high).

| Eval | Mode | Correctness | Workflow | Agent seconds | Total tokens | Outcome |
|---|---|---:|---|---:|---:|---|
| manifest-publication-python | flareml | 12/12 | pass | 219.1 | 329398 | completed |
| manifest-publication-python | tla | 12/12 | pass | 284.8 | 369212 | completed |
| manifest-publication-python | baseline | 12/12 | pass | 182.4 | 82465 | completed |

## Totals (descriptive, not a statistical estimate)

| Mode | Passing implementations | Successful executions | Cases | Agent seconds | Tokens including cache |
|---|---:|---:|---:|---:|---:|
| flareml | 1/1 | 1/1 | 12/12 | 219.1 | 329398 |
| tla | 1/1 | 1/1 | 12/12 | 284.8 | 369212 |
| baseline | 1/1 | 1/1 | 12/12 | 182.4 | 82465 |

## Paired differences (assisted minus baseline)

| Eval | Assisted mode | Token delta | Token ratio | Seconds delta | Time ratio |
|---|---|---:|---:|---:|---:|
| manifest-publication-python | flareml | 246933 | 3.99 | 36.7 | 1.20 |
| manifest-publication-python | tla | 286747 | 4.48 | 102.4 | 1.56 |

## Limitations

- One trial per cell: descriptive pilot, not a statistically supported treatment effect.
- Controlled storage/log services and asynchronous delivery between real processes; not a deployed network benchmark. See the task-specific clock and durability assumptions.
- Local Pi filesystem boundaries are instructions/directories, not an adversarial security sandbox.
- Caches and sequential execution order affect time/cost; treatment order is rotated.
- Provider usage includes reported cache tokens and reasoning inside output. A zero reported price is unavailable pricing, not free compute.
- Mode adherence is a smoke gate: shell-command keyword counts and saved complete reports, not exec tracing or proof adequacy. Supplemental model reruns and manual review are separate.
- Cases share a specification across languages; performance of the generated service is not benchmarked.

Run IDs, case failures, budgets, usage breakdown, and evidence locations are in `summary.json`. Raw EvalKit reports and Pi streams remain local and ignored by Git.
