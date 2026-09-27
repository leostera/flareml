# Public specifications as benchmark sources

Status: **source research and checker diagnostics**, not new agent results.
The existing problems, judges, and pilot results are unchanged. TLA+ mode,
Python targets, and the new distributed implementation judges still need to be
integrated before running the expanded campaign.

## 1. Checkpoint / backup coordination — strongest first distributed task

Source: [RingMaster checkpoint coordination](https://github.com/tlaplus/Examples/tree/c9e45d0e4695a8552d31105b93dff21456b7be69/specifications/CheckpointCoordination)
(MIT; pinned Examples revision). The accompanying README describes work for the
Azure DNS team, an implementation following the specification, and a twelve-step
TLC counterexample to a lease-preemption optimization.

**Why it fits:** independently operating replicas, leader failover, lagging log
application, crash/recovery, checkpoint activity, and lease ownership. It is not
just several frontends updating one atomic database record.

**Neutral task to extract:** implement a backup coordinator for a replicated
service. The primary must not checkpoint; at most one secondary may checkpoint;
healthy secondaries must eventually get turns after the system stabilizes;
started checkpoints must complete or abort. Replicas have private volatile and
durable state. Delivery, log application, recovery, and role changes are separate
events. No global candidate-controlled state transaction exists.

**Assumptions we must retain:** the source assumes an underlying replicated-log
service, caught-up leader eligibility, and particular heartbeat/time-drift
relationships. Do not ask for arbitrary-clock, arbitrary-partition safety plus
unconditional progress. The executable contract must specify clocks, fencing,
election/log guarantees, and post-stabilization fairness precisely. Do not quietly
make implementing consensus part of this task.

**Private validation targets:** overlapping checkpoint holders after failover and
replay; renewed authority from an obsolete lease; primary checkpointing; never
issuing work; unfair rotation. Published bug traces are private judge material,
not supplied to candidates.

**What was actually run:** the upstream failure configuration reproduced
`SafetyInvariant` failure after 1,302,999 generated / 88,152 distinct states in
about 12 seconds. The fixed configuration did **not finish** within 120 seconds;
its outcome is inconclusive, not verified. This is already a warning about model
size and the need to preserve the bug when choosing smaller bounds.
[Recorded diagnostic](../reports/source-research/checkpoint.json).

## 2. Replication with primary promotion and stale views

Source: [Elasticsearch data replication](https://github.com/elastic/elasticsearch-formal-models/blob/ca30663506a7e18de9d23fabbc70a369903d7c98/data/tla/replication.tla)
(Apache-2.0; pinned revision).

The source introduction explicitly covers sharding across machines, concurrent
replication, node crashes, per-request disconnects, delayed/out-of-order messages,
and asynchronous application of cluster state. A separate consensus-backed
master supplies allocation and primary terms. It explicitly omits shard recovery
and adding nodes; a failed shard remains unassigned in this model.

**Neutral task to extract:** a small replicated shard with independently running
primary/replica programs, local durable logs, monotonically changing terms,
delayed membership views, and client acknowledgements. Acknowledged writes must
survive the specified failover model, and stale-primary traffic must not corrupt
newer state. Constrain storage to per-node durability—no shared global CAS.

**Private validation targets:** acknowledgement before required durability,
accepting stale-term messages, losing acknowledged writes on promotion,
confusing received versus applied/committed positions, and unsafe log trimming.

**Scope discipline:** borrow the source's stated assumptions rather than claiming
to implement modern Elasticsearch or requiring unmodeled recovery. If we add
recovery, it is a separately documented benchmark extension with its own oracle.
The repository itself warns that models may differ from production designs.

## 3. Manifest publication and cleanup under uncertain I/O

Source: [Elasticsearch Storage.tla](https://github.com/elastic/elasticsearch-formal-models/blob/ca30663506a7e18de9d23fabbc70a369903d7c98/Storage/tla/Storage.tla)
(Apache-2.0; pinned revision). It includes `DeleteNewManifestBuggy`,
`DeleteNewManifestEasy`, and `DeleteNewManifestHard`, with links to
[issue 39077](https://github.com/elastic/elasticsearch/issues/39077) and
[fix 40519](https://github.com/elastic/elasticsearch/pull/40519).

**Why it fits:** a concrete artifact/metadata integrity failure, not a made-up
"concurrency is hard" exercise. It is, however, a single-control-flow storage
failure model—not by itself a multi-machine distributed coordination model.
Treat it as a crash-consistency intermediate tier, or explicitly justify and
independently validate a multi-publisher extension.

**Neutral task to extract:** publish immutable metadata generations and a
separately persisted manifest, then reclaim obsolete generations. An operation
may have taken effect even when its acknowledgement indicates uncertainty.
Recovery must yield an allowed committed/uncertain generation, and a readable
manifest must never reference missing metadata. Publication and deletion are
separate operations, not one transaction spanning both stores.

**Actual reproduction:** TLC release 1.7.4 (engine reports TLC2 2.19) verifies the
fixed path at `newMeta < 3` with 278 distinct states. Selecting the upstream buggy
cleanup path violates `MetadataFileReferencedByManifestExists` in a five-state
trace (29 distinct states explored):

1. Metadata is written.
2. Manifest write takes effect, but its result is uncertain.
3. Cleanup tries to remove the manifest, but removal does not take effect.
4. Cleanup nevertheless removes its metadata.
5. The surviving manifest now points to missing metadata.

This is a reproduction of an upstream-known regression, not a newly discovered
bug or evidence that an agent benefits from TLA+. The bounded check uses state
constraints and disables deadlock checking; it is not an unbounded or liveness
proof. [Recorded diagnostic](../reports/source-research/elastic-storage.json).

## Other sources, and why not start there

- [Elasticsearch cluster coordination](https://github.com/elastic/elasticsearch-formal-models/blob/ca30663506a7e18de9d23fabbc70a369903d7c98/ZenWithTerms/tla/ZenWithTerms.tla):
  directly tied by the repository README to its coordination implementation;
  checks one leader per term, log matching, quorum commitment, and ancestry of
  committed values. Good later task, but full reconfigurable consensus can dwarf
  a short coding budget.
- [Ongaro's Raft specification](https://github.com/ongardie/raft.tla/tree/6ecbdbcf1bcde2910367cdfd67f31b0bae447ddd):
  CC-BY-4.0, dissertation-linked. Useful independent reference; the README points
  to additional changes for TLC execution. Do not assume the original file is a
  turnkey executable benchmark. Familiarity/memorization is a significant risk.
- Elasticsearch's `ReplicaEngine` model explicitly says individual document
  operations run under a document lock. It contains interesting internal
  concurrency, but is a weaker starting point for the user's multi-machine goal
  than the replication model above.

## Extraction and fairness rules

- Candidates receive **only neutral requirements and the wire/storage contract**;
  no original TLA+/PlusCal model, existing implementation, tests, published bug
  trace, solution section, or task-specific FlareML translation.
- Keep source URLs, commits, licenses, and an assumptions/differences ledger in
  evaluator provenance. Preserve notices when copying licensed material. Do not
  infer permission to redistribute every external submodule from a collection's
  top-level license.
- All conditions get identical requirements and architecture constraints.
  Algorithm names/source links are not needed in candidate prompts. Public-source
  training contamination remains possible and must be disclosed.
- References inform the contract; they are not automatically correct or complete
  judges. Validate independent executable oracles with correct implementations,
  known-bad implementations, source counterexamples, and new schedules.
- Measure completed useful work as well as safety. Failing closed forever must
  not pass. Liveness checks apply only under the explicitly promised environment.
- Keep simple KV tasks as controls. Do not manufacture restrictions solely to
  obtain a favorable FlareML result.

## Expanded comparison to implement

Four target languages: **Rust, Go, TypeScript/Bun, Python 3** (standard library
runtime; independently launched processes; flushed JSONL for Python too).

Three modes:

1. **Baseline:** ordinary implementation, design reasoning and tests; no formal
   model checker.
2. **FlareML:** author a model from the requirements, check/refine it, preserve
   models/verdicts/traces, then implement and test.
3. **TLA+:** author TLA+ or PlusCal plus TLC configurations from the same
   requirements, check/refine them, preserve generated TLA+, configurations,
   verdicts and traces, then implement and test. No supplied upstream solution.

That is **12 cells per problem per trial**. The existing three problems would be
36 cells in the expanded matrix; their original 18-cell pilot remains untouched.
New distributed tasks add another 12 cells each. Use EvalKit for agent trials;
the source-check scripts here are only provider-free research diagnostics.

Keep the same provider/model, thinking, fresh workspace, overall time/token
budgets, and independent implementation judges. Include modeling, compilation,
translation, failed checks, and implementation testing in costs. Both formal
modes need generic quickstarts and bounded tool execution. Do not impose equal
state counts across different encodings as if that were equal modeling coverage.

Record these separately:

- parser/type errors;
- safety/liveness counterexamples;
- intentional reachability witnesses (a negated reachability invariant failing
  in TLC is **not** a discovered design bug);
- timeout, memory exhaustion, or incomplete search;
- changed assumptions, bounds, and properties;
- counterexample-driven model repair, subsequent implementation change, and an
  executable regression test derived from the witness;
- implementation correctness, workflow completion, and model adequacy.

A TLA+ comparison tests the current **agent + tool + documentation workflow**, not
just intrinsic expressiveness. Familiarity with TLA+, JVM startup, state-space
encoding, and documentation quality all affect that result.

## Reproduce the research diagnostics

From `evals/`, with Java installed and network access for pinned downloads:

```sh
bun scripts/check-storage-source.ts
bun scripts/check-checkpoint-source.ts
```

Sources, original licenses, configuration files, and raw TLC traces are retained
under ignored `_evalkit-evidence/source-research/`. The second command exits
nonzero if either expected result is not obtained, including its fixed-model
timeout. These commands make **no model-provider calls**.
