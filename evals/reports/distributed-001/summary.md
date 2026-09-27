# FlareML-assisted coding pilot: distributed-001

Status: **superseded-diagnostic**. 13/24 cells recorded. Model: `openai-codex/gpt-6-luna` (high).

| Eval | Mode | Correctness | Workflow | Agent seconds | Total tokens | Outcome |
|---|---|---:|---|---:|---:|---|
| checkpoint-coordination-rust | baseline | 12/12 | pass | 203.9 | 59584 | completed |
| checkpoint-coordination-rust | flareml | 12/12 | pass | 278.7 | 391092 | completed |
| checkpoint-coordination-rust | tla | 12/12 | pass | 347.9 | 624390 | failed / tokens |
| checkpoint-coordination-go | flareml | 4/12 | pass | 322.2 | 442982 | completed |
| checkpoint-coordination-go | tla | 12/12 | pass | 342.5 | 623458 | failed / tokens |
| checkpoint-coordination-go | baseline | 0/12 | pass | 168.6 | 89215 | completed |
| checkpoint-coordination-typescript | tla | ?/? | pass | 358.8 | 619840 | failed / tokens |
| checkpoint-coordination-typescript | baseline | 12/12 | pass | 215.3 | 176846 | completed |
| checkpoint-coordination-typescript | flareml | 12/12 | pass | 264.8 | 263593 | completed |
| checkpoint-coordination-python | baseline | 12/12 | pass | 174.9 | 87802 | completed |
| checkpoint-coordination-python | flareml | 12/12 | pass | 215.5 | 215611 | completed |
| checkpoint-coordination-python | tla | 12/12 | pass | 579.9 | 591661 | completed |
| manifest-publication-rust | flareml | 12/12 | pass | 370.1 | 627683 | failed / tokens |

## Totals (descriptive, not a statistical estimate)

| Mode | Passing implementations | Successful executions | Cases | Agent seconds | Tokens including cache |
|---|---:|---:|---:|---:|---:|
| baseline | 3/4 | 4/4 | 36/48 | 762.7 | 413447 |
| flareml | 4/5 | 4/5 | 52/60 | 1451.4 | 1940961 |
| tla | 3/4 | 1/4 | 36/36 | 1629.1 | 2459349 |

## Paired differences (assisted minus baseline)

| Eval | Assisted mode | Token delta | Token ratio | Seconds delta | Time ratio |
|---|---|---:|---:|---:|---:|
| checkpoint-coordination-rust | flareml | 331508 | 6.56 | 74.8 | 1.37 |
| checkpoint-coordination-rust | tla | 564806 | 10.48 | 144.0 | 1.71 |
| checkpoint-coordination-go | flareml | 353767 | 4.97 | 153.7 | 1.91 |
| checkpoint-coordination-go | tla | 534243 | 6.99 | 173.9 | 2.03 |
| checkpoint-coordination-typescript | flareml | 86747 | 1.49 | 49.5 | 1.23 |
| checkpoint-coordination-typescript | tla | 442994 | 3.50 | 143.4 | 1.67 |
| checkpoint-coordination-python | flareml | 127809 | 2.46 | 40.6 | 1.23 |
| checkpoint-coordination-python | tla | 503859 | 6.74 | 405.0 | 3.32 |

## Limitations

- One trial per cell: descriptive pilot, not a statistically supported treatment effect.
- Controlled storage/log services and asynchronous delivery between real processes; not a deployed network benchmark. See the task-specific clock and durability assumptions.
- Local Pi filesystem boundaries are instructions/directories, not an adversarial security sandbox.
- Caches and sequential execution order affect time/cost; treatment order is rotated.
- Provider usage includes reported cache tokens and reasoning inside output. A zero reported price is unavailable pricing, not free compute.
- Mode adherence is a smoke gate: shell-command keyword counts and saved complete reports, not exec tracing or proof adequacy. Supplemental model reruns and manual review are separate.
- Cases share a specification across languages; performance of the generated service is not benchmarked.

Run IDs, case failures, budgets, usage breakdown, and evidence locations are in `summary.json`. Raw EvalKit reports and Pi streams remain local and ignored by Git.
