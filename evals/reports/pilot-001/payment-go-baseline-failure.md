# Baseline Go payment failure: protocol framing

Original run: `6ad8fd6c-d50d-44ad-8638-d00c8994d58e`  
Original trial: `e4a2e390-1908-453a-a49e-89c972e2cb9c`

The agent completed normally and the project compiles, but every one of the 24
frozen payment schedules fails immediately with `init must return []`. These are
24 scenarios blocked by **one protocol defect**, not 24 distinct ledger bugs.

Independent reproduction against the original evaluator-built binary:

Input:
```jsonl
{"kind":"init","config":{"balance":10},"worker":0}
```

Actual stdout:
```jsonl
null
```

Required stdout is `[]`. The worker declares `var actions []Action`, leaves it nil
in the `init` branch, and calls `json.Marshal(actions)`. Go serializes a nil slice
as `null`, not an empty JSON array. The helper only substitutes `[]` on a marshal
error; marshaling nil succeeds.

The candidate has not been repaired, rerun as a replacement trial, or silently
normalized by the judge. This differs from the assisted Go worker's lost pending
request after initialization. Equal case-level scores do not imply identical
failure modes or repair costs, and these results do not establish that either
mode generally writes worse Go.
