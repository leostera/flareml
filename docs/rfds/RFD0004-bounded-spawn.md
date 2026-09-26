# RFD0004 — Bounded dynamic actor spawning

**Status:** design sketch; not implemented. Requires completion and validation of [RFD0003 (choice)](RFD0003-nondeterministic-choice-and-faulty-links.md). Refine this sketch before implementation.

**Next:** [RFD0005 (suspension and reentrancy)](RFD0005-suspension-and-reentrancy.md) is a separate milestone, not part of spawn.

## Motivation

A coordinator should be able to receive work, create a participant to perform it, and send that participant a message. Unlike current keyed actors, the participant does not exist at initialization: its existence and identity become part of reachable execution state.

This is dynamic creation in a finite experiment, not an unbounded runtime or an operating-system process launch. Actors remain concurrent participants and, for this milestone, each whole handler remains one non-reentrant atomic turn.

## Proposed direction

Illustrative handler syntax, not executable today:

```text
let worker = spawn(Worker);
send(worker, job);
```

`worker` is an `Actor<Worker>` reference. Each successful spawn produces a fresh identity with independent owned state and mailbox. References can be retained and transferred through messages under existing typing rules.

The sketch proposes:

- a finite lifetime creation pool per spawnable actor definition, configured by the check;
- no termination, deallocation, or identity reuse in the first implementation;
- deterministic fresh-slot allocation within each branch, rather than arbitrary selection of an unused slot;
- pure initialization and atomic publication with the creating turn;
- transitive allocation effects, disallowed in properties and initializers;
- no implicit fairness of optional requests to create actors.

A deterministic allocator defines identity-generation behavior; it is not a symmetry reduction. Allocation order must remain in state equality and replay. Do not expose incidental host pointers or random UUIDs as model identities.

## Definitions versus instances: resolve before implementation

Today `actor Worker { ... }` means both a definition and an eagerly created singleton. A keyed declaration similarly creates its full population. Blindly adding `spawn(Worker)` would leave an unexplained implicit singleton alongside dynamic instances.

Design an explicit distinction between a spawnable definition and existing static populations. Decisions required:

1. Does a spawnable definition use a declaration marker, or does the check define which declarations have initial instances?
2. Are singleton/keyed forms preserved as clear static-population shorthand, or replaced by explicit initial instance declarations?
3. What exactly does a bare `Worker` expression mean for a spawn-only definition? It must not silently select an arbitrary live instance.
4. How is initialization supplied: no arguments initially, or finite typed arguments to `spawn`? Are self references available to initializers or handlers?
5. How do actor reference types distinguish definitions without confusing them with allocated values?

Do not choose a source spelling for these merely to preserve existing parser structure. There are no legacy-language compatibility requirements, but any replacement must migrate existing examples and keep one coherent execution contract. This RFD does not depend on top-level constants or imports being implemented first.

## Atomic allocation and visibility

Proposed commit semantics:

1. Begin with the actor registry, allocation counters, state, and mailboxes of the pre-turn state.
2. Reserve fresh slots locally in source order as `spawn` executes. Different choice branches have independent reservations.
3. Initialize each proposed instance deterministically using permitted finite values. Initializers cannot send, spawn, choose, or inspect other actors' state.
4. Permit later statements in that same turn to retain the new reference and stage sends to it.
5. At successful commit, install the new instances and mailboxes, apply sender state, and publish the complete outbox and observations atomically.

A new actor cannot run before the creator commits. No other participant can observe a reserved-but-uncommitted identity. If initialization, allocation, or any staged send exceeds a bound, no part of the creating transition is published. Retry of an uncommitted branch must not consume identity slots globally.

Clarify stateless spawnable actors, initial state arguments, self-sends, and helper calls as part of the refined design. Do not add suspended initialization or implicit startup messages in this milestone.

## Finite bounds are not application behavior

The check declares finite creation capacity. Resolve the concrete spelling, whether zero is accepted, and how static instances contribute to total address limits before implementation.

Required semantics:

- Pool exhaustion or total host-address exhaustion makes exploration incomplete/inconclusive, unless a valid violation is already established.
- Do not return an implicit `None`, silently disable the spawning turn, or wrap/reuse an identity to close the graph.
- The pool is a lifetime creation bound, not merely a concurrent-live-actor limit, since this version has no destruction.
- An application policy such as “reject the eleventh job” must be ordinary model logic. The exploration guard must not masquerade as that rejection policy.
- Raising a bound allows additional executions; verification at one finite scope is not a proof for all population sizes.

A coordinator that limits its own creation requests can have a complete finite graph. A coordinator that keeps spawning forever will hit any finite lifetime pool; a cutoff is the honest outcome.

## Identity, state equality, and fairness

Include allocation state and live instances in exact state equality. Two states that currently have identical application fields but different remaining identity pools are not equivalent.

FIFO remains per address, and weak mailbox fairness attaches to the allocated typed identity. An unborn instance has no enabled processing action. After commit, its mailbox follows the same scheduling contract as existing instances. Fresh allocation must not recycle an old fairness identity.

`message_bound` currently means lifetime observations per actor declaration across all keys. The natural extension is across all static and dynamic instances of the definition, without per-spawn reset; finalize and document this alongside the identity representation.

## Properties over changing populations

A changing live-actor list must not be treated as a stable temporal quantifier domain. Otherwise a clause expanded at initialization could permanently omit every future worker.

Proposed direction: expose a finite, stable observation domain of potential instance slots, analogous to message slots. A slot indicates whether it has been created and carries an optional typed reference once allocated. Allocation is monotone for this version.

The refined design must specify:

- observation syntax and whether the view includes static instances;
- safe observation of owned state before creation, without inventing a default state;
- whether references allow read-only specification state lookup, since current state access is tied to static names/keyed targets;
- temporal binding stability and how a later-created actor can be mentioned from initialization;
- semantics for external input targets: the first version should keep declared input slots targeted at initially existing actors, with coordinators forwarding work to spawned actors.

Properties should be able to express “every worker that receives a job eventually finishes” without either dereferencing nonexistent state or vacuously ignoring workers created later. Unsupported forms must be rejected, not silently approximated.

## Evidence and replay

A spawn-capable trace must record or unambiguously reconstruct allocations, their order, initialization arguments, typed identities, and resulting registry/mailboxes. Extend and version the trace schema rather than overloading source descriptions with unchecked text.

Replay must check freshness, pool membership, allocation order, initialization, all references, and atomic publication. Reject future references, reused identities, missing instances, changed bounds, and fabricated allocation counters. Choice transcripts from RFD0003 and allocation evidence must compose within one turn.

Source snapshots and run reports must include enough configuration to reproduce the finite population scope. The artifact version is not a runtime-semantics selector.

## Example milestone

Build a bounded coordinator/worker model:

- Initially only the coordinator and any explicit static participants exist.
- Each accepted job creates a fresh worker and sends work to it.
- Workers send explicit completion messages carrying correlation identifiers.
- Safety checks distinct allocations, correct routing, and no completion for the wrong job.
- Reachability demonstrates at least two created workers receiving work and completing.
- With weak progress and finite fault-free work, accepted jobs complete; optional external submissions remain optional.
- An undersized creation pool yields inconclusive, not verified rejection or apparent deadlock.

Combine choice and spawn in a small regression, not by expanding this into supervision, cancellation, or a general task framework.

## Acceptance sketch

- [ ] Resolve declaration/instance syntax, initialization/self-reference rules, pool accounting, and observation syntax.
- [ ] Define the complete transition relation before modifying allocation code.
- [ ] Type/effect-check direct and transitive spawn, including prohibited initializer/property uses.
- [ ] Implement branch-local allocation and rollback with source-ordered sends to newly created actors.
- [ ] Preserve registry/counter state in interning, fairness, and stable property observations.
- [ ] Version and validate allocation evidence during replay.
- [ ] Independently enumerate tiny allocation/request workloads and compare every reachable transition.
- [ ] Test two spawns in one turn, interleaved coordinators, choice branches, pool exhaustion, initializer failure, and mailbox overflow after allocation.
- [ ] Test properties concerning instances created only after initialization; reject unsupported unstable quantification.
- [ ] Add coordinator/worker examples, per-property expected verdicts, artifact and corruption tests, and fuzz seeds.
- [ ] Record performance/cutoff behavior as pools grow; retain honest finite-scope claims.

## Deferred

Termination, identity reuse, supervision, process migration, restart/durability, unbounded creation, shared-memory capabilities, suspension, and reentrant request processing. None is implied by the word `spawn`.
