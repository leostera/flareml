---
title: For TLA+ users
description: Translate TLA+ state-machine ideas into FlareML's finite actor, mailbox, and scheduling model.
---

TLA+ and FlareML both let you describe state transitions and check temporal claims. The key difference is **who defines a transition**: a TLA+ specification supplies its own next-state relation; FlareML supplies a fixed actor runtime that submits inputs and schedules atomic message handlers.

## Map the modeling concepts

| TLA+ idea | FlareML starting point | Important difference |
| --- | --- | --- |
| `Init` | The selected check's deterministic `main` setup, actor `init` functions, and captured input slots | A model starts with no instances. `main` explicitly spawns the initial population and can enqueue guaranteed setup messages; there is no arbitrary initial-state predicate. |
| `Next` action | One input submission or one actor handler turn (which may spawn bounded instances) | There is no user-defined global next-state relation. A handler sees its actor's old state and atomically commits its next state, new instances, and outgoing sends. |
| `UNCHANGED` / stuttering | The checker always permits stuttering | A semantically meaningful idle/tick event should be modeled explicitly, not confused with scheduler stuttering. |
| `[]P` invariant | `property "name" { always P }` | `always` checks every reachable state in the configured finite experiment. |
| `<>P` | `eventually P` | This is a liveness claim over allowed infinite executions, with the declared fairness assumptions. |
| `P ~> Q` | `P leads_to Q` | The supported temporal fragment is limited; it is not the full TLA+ temporal language. |
| `WF_vars(A)` | Sometimes `fairness { weak runtime.progress }` | This only prevents starvation of continuously enabled mailbox processing. It does not impose fairness on inputs, particular actors/actions, or `choose` alternatives. |

Treat these as conceptual correspondences, not a syntax-level translation. In TLA+, you decide the full action relation and often model a queue as ordinary variables and actions. In FML, queue ordering, handler invocation, and atomic send publication are part of the engine. If your `Next` action combines several externally visible stages, decide whether it is truly one FML handler or whether the intermediate messages and interleavings need to be represented separately.

## Finite scope is part of the model

FML data pools, actor populations, and mailboxes are finite. For example, `domain Int = 0..3` gives exactly those integer values, while `spawn_bound Worker = 4` caps the lifetime number of Worker instances created by setup and handlers together. It is not a cutoff that preserves arbitrary integers or populations. Mailboxes also have explicit finite capacity; exhausting a bound makes exploration inconclusive rather than silently blocking or dropping a transition.

TLC can also check finite instances of TLA+ specifications, but its constants, symmetry sets, and state variables are not interchangeable with FML's data domains, explicit `main` setup, `spawn_bound`s, mailbox capacity, or runtime. Re-state the initial population and scope in the target model; do not assume the old configuration carried over.

## Fairness needs special care

A TLA+ spec may declare fairness over one or more actions. FML has one optional weak scheduling assumption: a mailbox-processing action that stays continuously enabled cannot be postponed forever. Optional external submissions are never required, and mailbox fairness does not force one branch of handler-local `choose`. A liveness claim that depended on action fairness in TLA+ may therefore need a different FML model or may not be expressible with the current scheduling contract.

## A practical port

1. Pick one TLA+ invariant or leads-to property.
2. Assign each state component to an actor, and identify messages that represent communication between components.
3. Break `Next` into deterministic setup, optional input submissions, and handler turns; use `spawn` for explicit creation and retain atomicity only where it is a justified modeling assumption.
4. Declare finite data pools, actor identities, input slots, mailbox bounds, and fairness explicitly.
5. Express the claim using the supported FML property forms, then compare the counterexample or witness with the intended TLA+ behavior.

Start with the [execution-model guide](/guide/execution-model/) and the [properties reference](/reference/properties/). For a model with explicit message delivery choices, see the [faulty-link bug and repair](https://github.com/leostera/flareml/tree/main/examples).
