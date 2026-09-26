---
title: For Quint users
description: Map Quint's executable state-machine specifications to FlareML actors, messages, and atomic turns.
---

Quint and FlareML are both intended for writing and exploring executable specifications, so the transition from one to the other can feel close. The biggest change is that FlareML gives you a **built-in actor runtime**: named finite addresses, one FIFO mailbox per address, optional input submissions, and atomic message-handler turns. A Quint model defines its own state and transition operators; an FML model expresses behavior through actors and the runtime's scheduling rules.

## Map state and transitions

| Quint model element | FlareML starting point | What to revisit |
| --- | --- | --- |
| Initial state / `init` | The check's deterministic `main` setup plus actor `init` functions | A model starts empty; `main` explicitly spawns initial participants and may register optional inputs or enqueue guaranteed setup messages. |
| State variables | The private state returned and accepted by each actor handler | A handler can directly read its own old state, not another actor's state. Cross-actor reads should usually become messages and later turns. |
| A transition/action | An input submission or one handler turn, which may create bounded instances | A handler turn atomically commits that actor's new state, any spawned instances, and staged sends. Split the model where real interleaving can occur. |
| Nondeterministic alternatives | Handler-local `let outcome = choose([...]);` | `choose` explores all compatible finite candidates, but only in a handler (or handler-only helper) binding. It is not a general-purpose effect or random choice. |
| Temporal claim | A supported FML property such as `always P`, `eventually P`, or `P leads_to Q` | The supported temporal fragment and fairness semantics are narrower than a general temporal logic. |

This is not a mechanical rewrite of a Quint `step`. A Quint transition can update the whole model state as one action. FML's runtime instead chooses one enabled input or one nonempty mailbox to process at a time, and an actor cannot synchronously inspect or update another actor. If a transition in Quint represents a multi-party operation, decide whether the FML version needs a request, response, and additional turns to expose the relevant interleavings.

## Make assumptions explicit

External input slots are optional; weak fairness does not force them to be submitted. Weak fairness only prevents permanent starvation of a mailbox-processing action that remains continuously enabled. Handler-local `choose` branches are exhaustive but unfair: mailbox fairness does not force a particular branch. These details can change progress claims even when the state updates look similar.

FML also requires finite data domains, a `spawn_bound` for every actor definition, and finite mailbox capacity. Each bound covers initial setup and later handler creation; identities are not reused. It explores the explicit state graph without symbolic or partial-order reduction. If a domain, creation pool, mailbox, or search budget is exceeded, the result is inconclusive. A completed result applies to the exact finite model—not to an unbounded Quint model or the implementation it describes.

## A migration workflow

1. Start with one Quint invariant or reachability/liveness question.
2. Partition global state by ownership: which actor owns each value?
3. Use `main` and `spawn` to create the initial population; turn cross-owner operations into typed messages and separate turns where an interleaving matters.
4. Declare optional external inputs, finite pools, mailbox bounds, and scheduling assumptions.
5. Translate the property into the supported FML fragment, then inspect and replay any counterexample.

The [execution-model guide](/guide/execution-model/) describes scheduling and atomicity; the [syntax](/reference/syntax/), [actors](/reference/actors/), and [properties](/reference/properties/) pages document the exact language contract. The [faulty-link examples](https://github.com/leostera/flareml/tree/main/examples) show how explicit nondeterministic outcomes expose a duplicate-delivery bug.
