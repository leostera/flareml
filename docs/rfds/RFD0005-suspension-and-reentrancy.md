# RFD0005 — Explicit suspension, atomic boundaries, and reentrancy

**Status:** design sketch with blocking semantic questions; not implemented. Implement only after [RFD0003 (choice)](RFD0003-nondeterministic-choice-and-faulty-links.md) and [RFD0004 (bounded spawn)](RFD0004-bounded-spawn.md) have been validated.

**Priority:** [RFD0006 (trace explorer)](RFD0006-trace-explorer.md) is the next implementation milestone. Resume this design after the current atomic execution contract has an interactive evidence viewer.

## Motivation

Actors are concurrent participants. The checker explores interleavings; physically running interpreter code in parallel is neither necessary nor sufficient to model concurrency correctly.

Today one entire handler is atomic. Modeling an operation that permits intervening activity requires splitting it into protocol messages and storing intermediate data in actor state. That is useful, but can obscure the algorithm when the intended operation is a single request with several visible steps.

Allow explicit suspension boundaries without silently making every expression interruptible. Keep one-message-at-a-time processing as the default. Separately consider opt-in reentrancy for models where one request can suspend and another request can execute on the same participant.

The analogy to PlusCal is control over observable atomic steps, not identical language or execution semantics. This is not an adapter contract for a particular event loop, isolate, service, or hosting platform.

## Terms and intended defaults

- **Invocation:** processing one dequeued message, potentially across several steps.
- **Segment:** execution from invocation start/resumption to an explicit suspension or completion boundary; atomic with respect to other modeled transitions.
- **Suspended frame:** retained local bindings and continuation/control position for an incomplete invocation.
- **Non-reentrant actor (default):** at most one active invocation. While it is suspended, other actors may run, but another message cannot start on this actor.
- **Reentrant actor (explicit opt-in):** another queued message may start while a prior invocation is suspended. Segments on that actor still interleave; they do not mutate the same actor state simultaneously within one transition.

Concurrency between actors and reentrancy within one actor are distinct. `async` must not be shorthand for unspecified scheduling behavior.

## Proposed shape, not settled syntax

The initial candidate is an explicit yield/suspend operation or labeled boundary inside handlers. The portion between boundaries remains atomic. An actor-level declaration would explicitly permit reentrancy if supported.

Do not commit to `await`, `yield`, or `atomic { ... }` spelling yet. In particular:

- An `atomic` block is insufficient unless the semantics of statements outside it are defined.
- A scheduler yield and waiting for a protocol reply are different operations.
- No implicit RPC, generated reply message, promise system, timeout, or I/O should be inferred from suspension.
- Arbitrary source-line scheduling boundaries would make formatting change the model; boundaries must be structural.

A minimal first sub-milestone may support only an always-resumable scheduler yield with non-reentrant actors. Condition-based waiting and reentrancy should not ship until their state and fairness rules are specified and tested. The RFD should describe their intended relationship even if implementation is staged.

## Blocking decisions

### 1. What commits at a boundary?

Current handlers receive a state snapshot and return a replacement state at completion. Local `let` bindings do not mutate actor-owned state. Therefore merely inserting a yield cannot implicitly determine a new owned state to publish.

Select an explicit model for publishing owned state at intermediate boundaries. Possibilities include supplying a next-state value at suspension, or introducing a separate explicit owned-state update operation. Evaluate them against existing state-in/state-out handlers before choosing syntax.

Proposed invariants regardless of spelling:

- A segment's selected state updates, sends, allocations, and frame change commit together.
- Earlier committed segments remain visible if a later segment reaches a checking cutoff; a whole multi-segment invocation is no longer one transaction.
- Failure during a segment never publishes half that segment.
- Outgoing messages may be processed after the publishing segment commits, even if the originating invocation is not finished.
- Unpublished local values remain private to the suspended frame.

Precisely define whether any effects may remain staged across suspension. Prefer no hidden cross-segment outbox or allocation reservation.

### 2. What happens to state when a frame resumes?

For a non-reentrant actor, other invocations cannot change its owned state while it is suspended. For a reentrant actor, they can.

The original state parameter is a snapshot, not a live reference. It must not silently become fresh on resume. Retaining an old snapshot and later returning it can overwrite another invocation's changes—the very bug a model may need to expose.

Decide how an invocation explicitly observes the actor's latest owned state after resuming. Clarify how that differs from retained local variables, whether access is allowed in helpers, and how final return merges or replaces state. Do not automatically merge records or refresh all locals to hide stale-read behavior.

### 3. When is resumption enabled?

An unconditional yield may always enable resumption after its boundary. A wait for a reply or condition needs a separately defined wakeup mechanism, correlation, and consumption rule.

In particular, a non-reentrant actor that waits for its ordinary message handler to process a reply cannot make progress while that handler is excluded. Either the protocol must avoid this pattern or the language must explicitly define a distinct wakeup delivery mechanism. Do not secretly allow another invocation to repair this deadlock.

Document whether a yielded actor may immediately resume if no other work runs; yielding alone must not promise another participant gets a turn without an applicable fairness assumption.

### 4. Which events count as message processing?

Today `inputs(A).processed` and `messages(A).processed` become true at the single handler commit. With suspension, distinguish queued, started, suspended/running, and completed.

Proposed meaning: `processed` remains completion of the whole invocation, not dequeue or first segment publication. If needed, expose a separate monotone `started` observation. Specify when a mailbox slot becomes available and where the in-flight message lives; pending-mailbox capacity and active-frame capacity are different bounds.

Stable identities must survive every suspension. Neither a frame nor a message observation may be recycled in a way that changes the referent of an existing temporal binding.

### 5. How is reentrancy bounded and declared?

Define an explicit per-actor declaration rather than a deployment profile or global semantics selector. Determine whether a language-level concurrency limit is needed separately from the host/check frame bound.

- A modeled limit can deliberately block starting another invocation; its scheduling semantics must be specified.
- Exhaustion of a checking/representation bound means incomplete exploration, not silently blocked work.
- FIFO governs which queued message starts next, but resumed invocations may finish in a different order.
- Frames must contain bounded, serializable local values and valid continuation locations.

### 6. What does weak progress mean now?

Mailbox-processing fairness alone no longer suffices: new invocations might keep completing while an older suspended invocation is never resumed.

Specify enabledness and fairness identities separately for message-start and frame-resume actions. Determine whether `weak runtime.progress` covers both, with stable identities that cannot be satisfied by unrelated work.

Waiting for a condition does not make that condition inevitable. Choice alternatives remain unfair unless separately designed; external submissions remain optional. Reuse the original-graph enabledness rule during temporal analysis, not property-filtered enabledness.

### 7. How do helpers and control flow suspend?

Decide whether suspension is initially restricted to the handler body. Allowing it inside helpers requires bounded serializable call frames, continuation positions, and transitive suspension effects. A pure function must not unexpectedly become a scheduling boundary through an indirect call.

Define how matches, Result handling, local scope, choice transcripts, and spawn publication behave across segments. Do not build a second inconsistent evaluator for resumed code.

## Shared memory: explicitly investigate, not silently introduce

A global-state actor models a shared resource accessed by messages. It does not automatically reproduce single-step shared-memory reads/writes, compare-and-swap, or relaxed-memory behavior.

Use a tiny sequentially consistent mutual-exclusion algorithm as a design study. Compare a direct read/write transition specification with its global-state-actor encoding and identify added transitions and assumptions. That study should decide whether explicit shared cells need a separate RFD.

This milestone does not grant handlers arbitrary access to other actors' state, introduce global mutable variables by accident, or claim relaxed-memory support. Suspension and shared-memory access are separate design axes.

## Replay, bounds, and checker obligations

Suspended frames and their continuation/local state become part of exact graph state. Reentrant invocations require stable distinguishable identities. Bounds must cover active frames, retained values, and any call stacks as well as the existing mailboxes and actor population.

Trace steps represent committed segments, starts, resumptions, and completions according to the chosen transition relation. Record enough evidence to reconstruct which invocation advanced and which choice/spawn events occurred. Update the trace format; replay must execute the segments, not trust serialized frame contents.

Reject altered program counters, locals, frame identities, completion flags, boundary publication, and resumption order. A trace from an earlier atomic-handler contract must not be silently interpreted under the new contract.

Safety is evaluated at every reachable committed boundary. Properties do not observe interpreter-internal partial updates within a segment. Fair-lasso validation must account for suspended invocations and original enabledness. Cutoffs cannot fabricate completion or erase waiting frames.

## Example and validation milestones

1. **Cross-actor interleaving:** one invocation publishes a message, suspends, and another actor processes it before the original invocation completes.
2. **Default serialization:** two messages arrive at one actor; the second cannot start while the first is suspended. Show other actors can still progress.
3. **Reentrant stale-state bug:** two overlapping invocations read the same old state and later overwrite an update. Produce a replayable counterexample with explicit frame/local evidence.
4. **Repair:** use the specified non-reentrant or atomic-update discipline and verify the corresponding finite model. Do not imply that merely rereading a value repairs every protocol.
5. **Waiting and starvation:** distinguish a genuinely disabled wait from an enabled frame indefinitely neglected by scheduling; test fair and unfair verdicts independently.
6. **Composition:** publish a spawned worker in one segment, communicate with it, and resume later; compose choice without treating its alternatives as fair.

Use an independent small-step interpreter/oracle for tiny bounded programs to compare reachable states, transition labels, enabledness, and observation flags. Test source changes that insert/remove a boundary and demonstrate the intended change in interleavings. Add replay corruption tests for every new state component, and fuzz nearly-valid continuation artifacts.

## Acceptance sketch

- [ ] Resolve all blocking decisions above and write a complete small-step execution contract.
- [ ] Settle the minimal first sub-milestone and explicit source syntax; update this RFD before implementation.
- [ ] Define state publication, retained locals, in-flight messages, frame capacity, and wakeup behavior.
- [ ] Define non-reentrant defaults, any reentrant opt-in, and action-level fairness identities.
- [ ] Preserve current whole-handler behavior for handlers without suspension under the single new contract, not a legacy runtime mode.
- [ ] Implement one coherent segmented interpreter and exact serializable frame state.
- [ ] Version and independently validate replay, safety boundaries, and fair recurrent paths.
- [ ] Deliver the applicable example pairs, independent oracle checks, tamper regressions, and fuzz evidence.
- [ ] Measure frame/interleaving growth and report realistic cutoffs before considering reductions.
- [ ] Record the shared-memory design study and explicitly defer or separately propose that capability.

## Non-goals

Physical parallel execution as a semantic guarantee; implicit RPC or external I/O; deployment-specific adapters; hidden crash/retry behavior; durable continuations; preemption between arbitrary expressions; relaxed-memory semantics; compatibility profiles; or a claim of implementing PlusCal.
