# RFD0003 — Nondeterministic choice and faulty links

**Status:** implemented. The [current RFD0002 contract](RFD0002-functions-and-actors.md) includes this extension. Ongoing independent review and longer fuzz campaigns remain part of stabilization, not a claim of proven correctness.

**Sequence:** implement and validate this RFD before [bounded spawn (RFD0004)](RFD0004-bounded-spawn.md), then [suspension and reentrancy (RFD0005)](RFD0005-suspension-and-reentrancy.md). Those sketches do not expand this milestone.

## Motivation

Actors are concurrent participants, broadly comparable to processes in an algorithm specification, not implementations of PlusCal semantics. Today scheduling and optional external inputs are nondeterministic, but a selected handler computes one outcome. Modeling a faulty link therefore requires awkward external control messages or auxiliary scheduling machinery.

Add a finite, explicit choice inside a turn. This makes protocol alternatives model code, rather than engine-selected transport profiles. Start with a small, testable semantic extension; no suspension, dynamic allocation, general shared memory, or standard-library packaging is required.

## Language

Implemented syntax:

```text
let outcome = choose([Deliver, Drop, Duplicate]);
```

`choose` is nondeterministic, not random and not probabilistic. Every candidate is a permitted outcome that the checker must account for. There are no weights and no implicit preference for the first candidate.

Initial surface:

- `choose([e1, ..., en])` appears as the complete right-hand side of a local `let` binding in a handler or handler-only helper.
- The list is a nonempty literal with one compatible element type. Its elements may be pure, state-dependent local expressions, not just constants. They must satisfy the existing finite-value rules.
- Evaluate the candidate expressions deterministically against that branch's current local environment. They cannot send, choose, spawn, or inspect global observations/state.
- The result has the element type. Existing rules still apply, including immediate explicit matching of a bound `Result`.
- Reject empty literals and incompatible element types as invalid models. An empty choice does not mean a blocked transition, stutter, or successful return.
- Preserve source order and candidate positions, including duplicate values. Duplicate values have no additional logical weight, but their positions are useful evidence for replay.
- Arbitrary computed collections, type-domain shorthand, and `choose` nested inside arguments, operators, records, or another choice are deferred. A binding makes the branching point explicit.
- Semicolons retain their normal meaning: `let picked = choose([...]); next_statement`.

This special operand does not generally enable specification collections or quantifiers as executable handler data. General collection manipulation is a separate design question.

### Effects

Add a transitive `chooses` effect, alongside the existing send/inspection effects. A helper that chooses is not pure even if it does not send. Permit calls to such helpers only in handler execution, as direct statements or binding initializers, following the existing effectful-helper placement discipline. For example, a helper may bind a choice and return the selected value.

Reject direct or indirect choice in:

- initializers and deterministic check setup;
- properties and specification helpers used by properties;
- finite-domain definitions and external input payload/target expressions;
- contexts that require pure expressions, including choice candidates.

Do not depend only on spelling at the immediate call site: aliases through ordinary helper calls must not evade effect restrictions.

## Execution semantics

A message turn remains atomic. Choice does not create a scheduling boundary or publish an intermediate state.

For a selected mailbox head:

1. Start from the same pre-turn state and message.
2. Execute local statements until a choice is reached.
3. Continue once per candidate, each with independent local bindings, proposed next state, and staged outbox.
4. Repeat for later choices reached on each branch. Branches may encounter different numbers of choices.
5. Each successfully completed branch produces a possible atomic processing transition: dequeue, next-state installation, sends, and completion observations commit together.

Two sequential binary choices normally give four local executions, not two. Sends before a choice must occur in every completed branch, once per branch; sends on one branch must never leak into another. Nested helper execution and branch-local bindings obey the same isolation rules.

Do not independently re-evaluate nondeterministic helpers while inspecting a state or evaluating a property. Only transition execution branches.

### Bounds and failure

Branch expansion introduces another source of host work and state explosion. The implementation bounds a turn to 128 choice encounters, 4096 candidates per encounter, and 4096 prefix executions, including incomplete prefixes. All attempts share the existing 100,000-entry evaluation work budget and nesting guards; the check deadline is polled during statements, candidate evaluation, expansion, and branch commit. Poll during expansion, not only between completed turns. A Cartesian product must not be fully allocated before checking its size/work bound.

A capacity/domain/evaluation cutoff in any permitted branch makes exploration incomplete. Never silently remove the branch and verify the remaining graph. An invalid model computation must retain the existing diagnostic classification; it is not converted into a modeled nondeterministic failure.

Atomic rollback applies separately to each branch. No failed branch leaves partially committed state, observations, or messages. Valid counterexamples and reachability witnesses already discovered remain evidence even when exploration is incomplete. The implementation need not enumerate all siblings after a decisive safety failure, but cannot call unexamined requirements verified.

This RFD does not introduce model-level blocked sends or backpressure. An application refusal must be an explicit modeled outcome, distinct from exhaustion of a checking bound.

## Scheduling and fairness

Keep weak fairness attached to mailbox processing identity, not to a selected candidate or a whole choice transcript. Processing any permitted outcome services that mailbox action.

There is **no fairness between choice alternatives**. An execution may choose `Drop` forever, even while the link processes every packet fairly. Conversely, if a turn containing `Drop` commits, the incoming packet's processing observation becomes true even though no outgoing packet exists.

Therefore neither reliable delivery nor eventual successful retry follows from weak mailbox fairness alone. Progress claims require appropriate explicit restrictions, such as a finite loss budget and a protocol that continues attempting delivery. A finite number of attempts alone does not guarantee success. New temporal operators and choice-fairness declarations are out of scope.

## Faulty-link example milestone

Model this topology explicitly:

```text
Sender -> FaultyLink -> Receiver
```

The engine still enqueues committed sends without loss into FIFO mailboxes. Eventual processing requires the applicable scheduling assumptions. The link decides whether to produce an onward send; loss and duplication are ordinary model behavior.

An illustrative handler branch is:

```text
let outcome = choose([Deliver, Drop, Duplicate]);
match outcome {
  | Deliver -> send(destination, packet)
  | Drop -> ()
  | Duplicate -> {
      send(destination, packet);
      send(destination, packet);
    }
}
```

The checked-in example must be a complete model with typed packets, explicit destination routing, finite bounds, and no external randomness. Use a fixed logical request identifier so duplicate reception can be distinguished from two independent operations.

Deliver these scenarios, preferably as small models rather than one large fixture:

1. **Delivery is possible:** a reached witness shows the link forwarding and the receiver committing.
2. **Loss defeats unconditional progress:** under weak mailbox fairness, submission need not lead to receiver completion. Save and replay the fair counterexample after the link drops the packet.
3. **Duplication breaks an unsafe receiver:** one logical operation can be applied twice. Demonstrate an actual safety failure, not merely a capacity cutoff.
4. **Idempotent repair:** remembering the request identifier at the modeled application boundary prevents duplicate application; completion remains reachable. Do not claim this provides durability or atomicity with a separate external API.

Select each intended property where necessary: an early safety failure must not mask a reachability or liveness scenario in the acceptance tests. Mailbox/domain bounds must allow the relevant duplicate execution; a cutoff is not the expected duplicate-operation verdict.

### Reordering and reuse

A link that immediately forwards packets from one FIFO cannot reorder them merely by choosing deliver/drop/duplicate. Reordering requires retaining packets and choosing which buffered packet to release in later turns. That introduces buffer state, a flush/tick protocol, and explicit enabledness/progress assumptions.

A bounded two-packet reordering example is a follow-up, not an implicit guarantee of the first example. A reusable faulty-link library can follow once its behavior is demonstrated. Imports, generic actor libraries, timers, and hidden transport modes are not prerequisites.

## Evidence and replay

Current trace actions identify deterministic turns. A nondeterministic turn needs explicit execution evidence:

- an ordered transcript of choice encounters;
- the source location and sufficient execution context to distinguish repeated helper calls;
- the selected candidate position, with the selected value either recorded and checked or deterministically reconstructed.

A source span alone is not an encounter identifier: the same helper can run multiple times in one turn. The choice layout was introduced in format **7** and is retained in current format **8**, which adds RFD0004 allocation evidence: every action includes `choices`, an ordered array of `{ encounter, span, calls, candidate, value }`. `encounter` and `candidate` are zero-based; `calls` is the ordered stack of helper call-expression spans. Selected values are recorded and checked against freshly evaluated candidates. Continue accepting only that current format, not a choice-enabled compatibility profile.

Replay must re-execute the turn with the transcript, check every candidate selection against the freshly evaluated candidate list, consume the transcript exactly, and compare the resulting state, sends, provenance, and observations. Reject missing, extra, reordered, out-of-range, or source-mismatched selections. Never validate a trace by rerunning an unconstrained choice and hoping for the same result.

Keep fairness identity separate from branch evidence. Equal successor states may be interned, but edge handling must not lose required action/fairness distinctions or produce unreplayable paths. Run bundles must persist all available witnesses using the new trace representation.

## Implementation details

`choices.rs` enumerates choice prefixes in source order through the same `eval_body` interpreter used by deterministic turns. At the first missing selection, evaluation requests an extension; each candidate yields a new prefix. Each attempt begins with fresh locals and an empty staged outbox. Re-executing deterministic prefixes trades speed for a small shared interpreter and explicit isolation; it is not a state-space reduction.

The whole per-turn enumeration is bounded before queuing further prefixes. Completed outcomes are committed through the existing atomic messaging path. As before, a successor-generation cutoff conservatively stops exploration; unreturned sibling outcomes are not treated as discovered witnesses. Previously discovered evidence is retained.

Replay supplies a complete transcript and executes only its selected branch. It rejects extra or missing selections and altered encounter/site/call context, position, or value. It compares the full resulting action and state. A different transcript that describes an equally valid execution is not intrinsically corruption; evidence is validated semantically, not cryptographically authenticated. Fair enabledness comes from nonempty mailboxes in the original state, without expanding unrelated choice outcomes during replay.

## Implementation and acceptance checklist

- [x] Reserve/type `choose` and enforce binding placement, nonempty literal operands, compatible types, and pure candidates.
- [x] Infer and enforce transitive choice effects, including indirect initializer/property/input uses.
- [x] Extend the shared local interpreter to enumerate branch-local executions without duplicating incompatible evaluator semantics.
- [x] Integrate all completed outcomes into successor generation, with explicit bounded work and truthful incomplete exploration.
- [x] Preserve mailbox fairness identity independently of chosen alternatives.
- [x] Version trace evidence and implement constrained replay with exact transcript consumption.
- [x] Add the complete faulty-link scenarios above, documented assumptions, expected per-property verdicts, and persisted witness replay.
- [x] Compare small choice programs with an independent enumeration oracle: nested choices, conditional encounters, helper calls, and multiple sends.
- [x] Test duplicates, malformed/empty operands, branch-local scope, immediate Result handling, and effect escapes.
- [x] Test outbox/state/observation isolation and bounds reached on one branch without false verification.
- [x] Test that fair processing does not force a favorable choice; replay the corresponding lasso.
- [x] Tamper every transcript dimension; include semantically valid but wrong alternative selections and repeated helper sites.
- [x] Add source/trace fuzz seeds, run stable checks and instrumented campaigns, and record actual evidence.
- [x] Measure choice-heavy examples, including state/edge counts and cutoffs. Make no reduction or scalability claims without evidence.
- [x] Update RFD0002/current documentation only when the behavior is implemented; record the trace version and host guard values.

## Recorded validation and measurements

The implemented examples are [loss](../../examples/faulty-link-loss.fml), [duplicate-application bug](../../examples/faulty-link-duplicate-bug.fml), and [idempotent repair](../../examples/faulty-link-duplicate-fixed.fml). `tests/faulty_links.rs` independently selects all eight intended obligations, checks verdicts, replays finite/lasso evidence, and corrupts persisted choice transcripts. `tests/choices.rs` compares 16 candidate-list pairs against independent Cartesian enumeration, checks dependent helper calls and outbox order, branching encounter counts, effect restrictions, Result handling, fairness, safety, cutoff preservation, and transcript tampering. Unit regressions check expansion deadline propagation/guard reset and compare replay fairness enabledness against exhaustive successor generation. Stable validation at this milestone: **147 tests passed**, formatting and strict all-target Clippy passed, and fuzz targets compile on stable.

Initial local coverage/sanitizer-instrumented campaigns on `aarch64-apple-darwin`, nightly, seed `12345`, with `-max_total_time=120 -timeout=10 -rss_limit_mb=2048` passed: **790,621 source executions** and **2,896,980 trace-JSON executions**, each in 121 seconds. Valid source and format-7 trace seeds included the faulty-link models; the trace fuzzer selects the matching source fixture by hash. These short campaigns are smoke evidence, not a coverage-completeness claim. CI seed generation was extended accordingly. Follow-up 120-second reruns were interrupted by the orchestration wall-time limit and are not counted as completed campaigns. Final instrumented smoke runs on the updated binaries completed **20,000 executions per target** (seed `67890`; source 4 seconds, trace JSON 1 second) without reported failures.

Exploration measurements with default graph budgets and no message-history bound:

| Model | States | Edges | Result / completeness |
| --- | ---: | ---: | --- |
| Faulty-link loss | 6 | 11 | Violated; closed graph |
| Duplicate-application bug | 9 | 16 | Violated; early safety exit |
| Idempotent repair | 9 | 17 | Verified in scope; closed graph |
| 1 independent binary choice | 4 | 7 | Verified in scope |
| 4 independent binary choices | 4 | 21 | Verified in scope |
| 8 independent binary choices | 4 | 261 | Verified in scope |
| 10 independent binary choices | 4 | 1029 | Verified in scope |
| 12 or 14 independent binary choices | 2 | 2 | Inconclusive: 100,000-entry evaluation guard |

The synthetic model uses one Boolean actor, one optional unit input, sequential `let xN = choose([false, true]);` bindings, returns the last binding, and checks `always true` with mailbox bound 1. Earlier choices are deliberately unused: equal resulting states do not remove alternative labeled edges. These are tiny bounded measurements demonstrating exponential local branching and honest cutoffs, not practical scalability claims for large protocols. Reordering, libraries, spawn, and suspension remain outside this milestone.
