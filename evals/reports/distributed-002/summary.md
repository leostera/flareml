# FlareML-assisted coding pilot: distributed-002

Status: **completed**. 24/24 cells recorded. Model: `openai-codex/gpt-6-luna` (high).

| Eval | Mode | Correctness | Workflow | Agent seconds | Total tokens | Outcome |
|---|---|---:|---|---:|---:|---|
| checkpoint-coordination-rust | baseline | 84/84 | pass | 314.1 | 104677 | completed |
| checkpoint-coordination-rust | flareml | 72/84 | pass | 330.4 | 360949 | completed |
| checkpoint-coordination-rust | tla | 72/84 | pass | 393.8 | 615588 | failed / tokens |
| checkpoint-coordination-go | flareml | 84/84 | pass | 224.8 | 318029 | completed |
| checkpoint-coordination-go | tla | 0/84 | pass | 360.1 | 611606 | failed / tokens |
| checkpoint-coordination-go | baseline | 84/84 | pass | 249.5 | 235405 | completed |
| checkpoint-coordination-typescript | tla | 84/84 | pass | 417.2 | 602277 | failed / tokens |
| checkpoint-coordination-typescript | baseline | 84/84 | pass | 263.4 | 172406 | completed |
| checkpoint-coordination-typescript | flareml | 84/84 | pass | 259.2 | 322157 | completed |
| checkpoint-coordination-python | baseline | 84/84 | pass | 212.6 | 86416 | completed |
| checkpoint-coordination-python | flareml | 84/84 | pass | 322.5 | 466960 | completed |
| checkpoint-coordination-python | tla | 84/84 | pass | 357.9 | 622299 | failed / tokens |
| manifest-publication-rust | flareml | 12/12 | pass | 245.6 | 362460 | completed |
| manifest-publication-rust | tla | 12/12 | pass | 275.8 | 273985 | completed |
| manifest-publication-rust | baseline | 12/12 | pass | 282.2 | 216214 | completed |
| manifest-publication-go | tla | 12/12 | pass | 271.3 | 329165 | completed |
| manifest-publication-go | baseline | 12/12 | pass | 228.5 | 235526 | completed |
| manifest-publication-go | flareml | 12/12 | pass | 307.6 | 315381 | completed |
| manifest-publication-typescript | baseline | 12/12 | pass | 179.8 | 56458 | completed |
| manifest-publication-typescript | flareml | 12/12 | pass | 315.9 | 342819 | completed |
| manifest-publication-typescript | tla | ?/? | fail | 128.2 | 5567 | failed |
| manifest-publication-python | flareml | ?/? | fail | 14.3 | 0 | failed |
| manifest-publication-python | tla | ?/? | fail | 14.3 | 0 | failed |
| manifest-publication-python | baseline | ?/? | pass | 14.3 | 0 | failed |

## Totals (descriptive, not a statistical estimate)

| Mode | Passing implementations | Successful executions | Cases | Agent seconds | Tokens including cache |
|---|---:|---:|---:|---:|---:|
| baseline | 7/8 | 7/8 | 372/372 | 1744.4 | 1107102 |
| flareml | 6/8 | 7/8 | 360/372 | 2020.3 | 2488755 |
| tla | 4/8 | 2/8 | 264/360 | 2218.7 | 3060487 |

## Paired differences (assisted minus baseline)

| Eval | Assisted mode | Token delta | Token ratio | Seconds delta | Time ratio |
|---|---|---:|---:|---:|---:|
| checkpoint-coordination-rust | flareml | 256272 | 3.45 | 16.3 | 1.05 |
| checkpoint-coordination-rust | tla | 510911 | 5.88 | 79.8 | 1.25 |
| checkpoint-coordination-go | flareml | 82624 | 1.35 | -24.7 | 0.90 |
| checkpoint-coordination-go | tla | 376201 | 2.60 | 110.6 | 1.44 |
| checkpoint-coordination-typescript | flareml | 149751 | 1.87 | -4.3 | 0.98 |
| checkpoint-coordination-typescript | tla | 429871 | 3.49 | 153.8 | 1.58 |
| checkpoint-coordination-python | flareml | 380544 | 5.40 | 109.9 | 1.52 |
| checkpoint-coordination-python | tla | 535883 | 7.20 | 145.3 | 1.68 |
| manifest-publication-rust | flareml | 146246 | 1.68 | -36.6 | 0.87 |
| manifest-publication-rust | tla | 57771 | 1.27 | -6.4 | 0.98 |
| manifest-publication-go | flareml | 79855 | 1.34 | 79.2 | 1.35 |
| manifest-publication-go | tla | 93639 | 1.40 | 42.8 | 1.19 |
| manifest-publication-typescript | flareml | 286361 | 6.07 | 136.1 | 1.76 |
| manifest-publication-typescript | tla | -50891 | 0.10 | -51.6 | 0.71 |
| manifest-publication-python | flareml | 0 | undefined | 0.0 | 1.00 |
| manifest-publication-python | tla | 0 | undefined | 0.0 | 1.00 |

## Limitations

- One trial per cell: descriptive pilot, not a statistically supported treatment effect.
- Controlled storage/log services and asynchronous delivery between real processes; not a deployed network benchmark. See the task-specific clock and durability assumptions.
- Local Pi filesystem boundaries are instructions/directories, not an adversarial security sandbox.
- Caches and sequential execution order affect time/cost; treatment order is rotated.
- Provider usage includes reported cache tokens and reasoning inside output. A zero reported price is unavailable pricing, not free compute.
- Mode adherence is a smoke gate: shell-command keyword counts and saved complete reports, not exec tracing or proof adequacy. Supplemental model reruns and manual review are separate.
- Cases share a specification across languages; performance of the generated service is not benchmarked.

Run IDs, case failures, budgets, usage breakdown, and evidence locations are in `summary.json`. Raw EvalKit reports and Pi streams remain local and ignored by Git.
