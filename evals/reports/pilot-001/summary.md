# FlareML-assisted coding pilot: pilot-001

Status: **completed**. 18/18 cells recorded. Model: `openai-codex/gpt-6-luna` (high).

| Eval | Mode | Correctness | Workflow | Agent seconds | Total tokens | Outcome |
|---|---|---:|---|---:|---:|---|
| url-shortener-rust | baseline | 20/20 | pass | 198.9 | 107958 | completed |
| url-shortener-rust | flareml | 20/20 | pass | 269.2 | 466070 | completed |
| url-shortener-go | flareml | 20/20 | pass | 425.3 | 631214 | failed / tokens |
| url-shortener-go | baseline | 20/20 | pass | 156.9 | 87235 | completed |
| url-shortener-typescript | baseline | 20/20 | pass | 189.4 | 123595 | completed |
| url-shortener-typescript | flareml | 20/20 | pass | 253.4 | 408480 | completed |
| inventory-rust | flareml | 20/20 | pass | 270.9 | 403625 | completed |
| inventory-rust | baseline | 20/20 | pass | 187.4 | 156123 | completed |
| inventory-go | baseline | 20/20 | pass | 175.9 | 109502 | completed |
| inventory-go | flareml | 20/20 | pass | 332.7 | 545394 | completed |
| inventory-typescript | flareml | 20/20 | pass | 264.0 | 427459 | completed |
| inventory-typescript | baseline | 20/20 | pass | 149.6 | 70556 | completed |
| payments-rust | baseline | 24/24 | pass | 141.9 | 90297 | completed |
| payments-rust | flareml | 24/24 | pass | 296.6 | 471226 | completed |
| payments-go | flareml | 0/24 | pass | 336.2 | 615280 | failed / tokens |
| payments-go | baseline | 0/24 | pass | 254.6 | 174684 | completed |
| payments-typescript | baseline | 24/24 | pass | 165.9 | 149001 | completed |
| payments-typescript | flareml | 24/24 | pass | 345.8 | 617024 | failed / tokens |

## Totals (descriptive, not a statistical estimate)

| Mode | Passing implementations | Successful executions | Cases | Agent seconds | Tokens including cache |
|---|---:|---:|---:|---:|---:|
| baseline | 8/9 | 9/9 | 168/192 | 1620.5 | 1068951 |
| flareml | 8/9 | 6/9 | 168/192 | 2794.1 | 4585772 |

## Paired differences (assisted minus baseline)

| Eval | Token delta | Token ratio | Seconds delta | Time ratio |
|---|---:|---:|---:|---:|
| url-shortener-rust | 358112 | 4.32 | 70.3 | 1.35 |
| url-shortener-go | 543979 | 7.24 | 268.4 | 2.71 |
| url-shortener-typescript | 284885 | 3.30 | 64.0 | 1.34 |
| inventory-rust | 247502 | 2.59 | 83.5 | 1.45 |
| inventory-go | 435892 | 4.98 | 156.9 | 1.89 |
| inventory-typescript | 356903 | 6.06 | 114.3 | 1.76 |
| payments-rust | 380929 | 5.22 | 154.7 | 2.09 |
| payments-go | 440596 | 3.52 | 81.6 | 1.32 |
| payments-typescript | 468023 | 4.14 | 179.9 | 2.08 |

## Limitations

- One trial per cell: descriptive pilot, not a statistically supported treatment effect.
- A fixed in-memory linearizable KV service and explicit async message schedules, not a deployed network benchmark.
- Local Pi filesystem boundaries are instructions/directories, not an adversarial security sandbox.
- Caches and sequential execution order affect time/cost; treatment order is alternated.
- Provider usage includes reported cache tokens and reasoning inside output. A zero reported price is unavailable pricing, not free compute.
- Mode adherence is a smoke gate: shell-command keyword counts and saved complete reports, not exec tracing or proof adequacy. Supplemental model reruns and manual review are separate.
- Cases share a specification across languages; performance of the generated service is not benchmarked.

Run IDs, case failures, budgets, usage breakdown, and evidence locations are in `summary.json`. Raw EvalKit reports and Pi streams remain local and ignored by Git.
