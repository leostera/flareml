# Distributed pilot v2: executable subset and preflight

**Historical diagnostic design.** The campaign was stopped following judge
review. See [distributed-002.md](distributed-002.md) for the corrected suite and
fresh full agent evaluations. Saved-candidate regrades are not the comparison.

This adds two problems without removing the original three problems or their
judges/results. All projects still start with SPEC.md only; formal modes add
only their tool and generic documentation. No upstream solution model is given.

## Exact scope (not a claim to reproduce whole upstream systems)

- **checkpoint-coordination:** three independent processes coordinate checkpoint
  leases through a provided fenced consensus log. Log application, append
  execution/results, view delivery, crashes/replay, and checkpoint work are
  distinct. The runtime cannot grant exclusion automatically: overlapping valid
  leases can result in overlapping work. The historical preemption/replay failure
  is a negative control. A stabilized suffix requires useful repeated work by
  both secondaries; never granting work fails.
- **manifest-publication:** two independent processes publish to separate blob
  and head-catalog services, with no cross-service transaction. Payloads cannot
  be stored in the catalog; its only value is a generation number. Concurrent
  publication, reads and collection encounter writes/deletes that may take effect
  despite uncertain results. Safety is checked after every physical mutation,
  not only after client replies. Reads/publications are checked against possible
  head values in their invocation intervals. Quiescent collection must reclaim
  obsolete blobs. Crashed incarnations lose replies but issued storage effects
  may still execute.

The checkpoint subset deliberately uses a **shared logical clock with runtime
checkpoint deadlines**, not the upstream model's clock-drift/heartbeat mechanism.
It isolates failover, stale authority and log replay; it cannot establish safety
under arbitrary clock skew or unreliable expiry enforcement. Consensus and leader
eligibility are provided, not implemented by the candidate. Checkpoint completion
is durably recorded by the runtime. Role promotion can interrupt local work.

The manifest subset adds concurrent publishers, monotonic externally assigned
versions and coherent readers to the source's single-writer failure example.
This is an adaptation, not a translation/equivalence claim. Future staged blobs
are not garbage; only versions below the monotonic head are collectible. No
production object store's actual guarantees are assumed implicitly.

## Judges and controls

Each problem initially registered three case labels under four fixed seeds: 12
cases per implementation. They are deterministic for a given emitted action
history, not identical event lists for algorithms with different actions.

During the checkpoint portion of the campaign, before any manifest candidate
was generated, review found that two manifest labels exercised the same workload.
The original manifest corpus therefore has 8 distinct workload/seed pairs, not
12 independent schedules. It also did not kill a stale-read-to-missing mutant.
The running campaign's loaded graders and original records are not hot-patched.
A separate, frozen review corpus replaces the duplicate group with schedules
that pause a blob fetch, publish a newer head, collect the old blob, then resume
the read. The correct reference passes; the stale-missing mutant fails all four
new schedules. Every manifest candidate will receive the same supplemental
EvalKit regrade, without model calls, source repairs, or replacement agent trials.
Both original and reviewed scores will be retained.

Private trusted workers are independent of the judging logic. Preflight checks:
- all 24 reference scenarios pass;
- premature lease preemption and refusing work fail;
- deleting a possibly committed publication, regressing the head, deleting the
  current blob and never collecting garbage fail;
- Python syntax/build plus the JSONL subprocess path works;
- the TLC runner records a real complete check and a real invariant violation.

These controls do not prove the judges complete. Human review and additional
held-out schedules remain important. Directory separation is not a security
sandbox. The subprocess protocol is the tested integration boundary, not TCP or
a deployed distributed service.

## Campaign

`bun scripts/campaign.ts --suite=distributed --id=distributed-001`

Two problems × four languages (Rust, Go, TypeScript/Bun, Python) × three modes
(baseline, FlareML, TLA+) = **24 sequential agent trials**. Each uses fresh local
Pi with `openai-codex/gpt-6-luna`, high thinking, 12 minutes, 80 responses and
600,000 reported total tokens including cache. Treatment order rotates across
language/task groups. No model or language is substituted after failures.

TLC release 1.7.4 is pinned by SHA-256. Its generic wrapper records source/config
hashes and raw output, enforcing 30 seconds/512MiB/one worker per invocation.
A completed check is only a workflow smoke gate, not property adequacy. Intentional
negated reachability invariants must not be counted as discovered design bugs.
The existing FlareML smoke gate remains separately labeled as heuristic.

All 60 combinations (original + added tasks, four languages, three modes) are
also discoverable by EvalKit's matrix CLI. The campaign's default suite is still
`simple`; `--suite=distributed` selects only the new tasks. To select the old
language/mode set, use `--languages=rust,go,typescript --modes=baseline,flareml`.
Old pilot artifacts remain immutable; extending the live suite does not rewrite
those results.
