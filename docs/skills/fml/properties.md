---
title: FML properties manual
description: "Safety, reachability, temporal claims, fairness, bounded populations, and counterexample evidence."
---

# FML properties manual

This page is bundled with your installed `fml` binary. Related: `fml skills observations` for `instances`/`inputs`/`messages`, `fml skills checks` for scope and fairness, and `fml skills cli` for reports and replay.

Every claim is written `property "name" { expression }`. The expression must state its meaning explicitly; bare state predicates are rejected. Properties are read-only and cannot declare a local `let` directly. Put local computation in a pure/specification helper. Actor state is observed by quantifying over `instances(A)`; actor definition names and routing references do not expose state. Parenthesize compounds, for example `always (predicate)`.

| Form (`P`, `Q` are state predicates) | Meaning | Failed claim evidence |
| --- | --- | --- |
| `always P` | P holds in every reachable state | Finite bad prefix; checked from the initial state onward |
| `reachable P` | At least one finite execution reaches P | `REACHED` witness, or `UNREACHABLE` on a complete graph |
| `eventually P` | Every relevant infinite execution eventually satisfies P | Repeating lasso when false |
| `P leads_to Q` | Every occurrence of P is eventually followed by Q | Repeating lasso |
| `P until Q` | P remains true until Q occurs; Q must occur | Bad prefix or lasso |
| `always eventually P` | P recurs infinitely often | Lasso |
| `eventually always P` | P eventually remains true | Lasso |
| `always (P implies always Q)` | Once P happens, Q remains true | Finite bad prefix |

Examples:

```fml
property "creation is possible" {
  reachable (exists (worker in instances(Worker)) { worker.created })
}
property "created workers finish" {
  forall (worker in instances(Worker)) {
    worker.created leads_to worker.state == Some(true)
  }
}
property "submitted input is processed" {
  forall (input in inputs(Counter)) { input.submitted leads_to input.processed }
}
```

`reachable P` is supported **only as a whole property body**; P may be a Boolean state predicate with finite data quantifiers. `reachable (eventually P)`, `always (reachable P)`, and mixtures of temporal and reachability claims are rejected. `exists (x in T) { predicate }` and `forall (x in T) { predicate }` quantify finite **data**, not paths. Temporal conjunction and stable universal quantification over data/observation slots are supported; arbitrary temporal nesting, temporal disjunction/negation, `next`, and strong fairness are not. `always eventually P` is temporal, not the fast safety case `always P`.

## Scope, creation, and fairness

`reachable P` means *possible*, not guaranteed. It is already reached if P is true initially. `eventually P` means required on every allowed execution (for liveness, under declared fairness). The checker always allows stuttering. Without fairness, an enabled mailbox can be postponed forever. `fairness { weak runtime.progress }` rules out paths that forever postpone a mailbox-processing action that stays continuously enabled; it does not force optional input submission, actor creation, or a favorable choice outcome.

`instances(A)` includes every potential creation slot from the initial state, including unborn instances. Therefore `forall (worker in instances(A)) { P(worker.state) }` may be vacuously true for a zero bound or before creation. Guard state claims with `worker.created`, handle `None`, and add a reachability property if creation itself matters. `inputs(A)` includes only external slots captured during setup; each is optional. Safety and reachability search all reachable states without using fairness to prune them. With handler-local `choose`, weak mailbox fairness does not force a favorable alternative: a faulty link can process fairly while dropping every packet.

Progress conditioned on submission is intentional: `forall (i in inputs(A)) { i.submitted leads_to i.processed }` does **not** claim every input is submitted. Processing only means the input's own callback committed, not that a reply arrived. For replies, inspect the response actor state or `messages(ReplyActor)` as documented in `fml skills observations`.

## Results and cutoffs

A whole `always P` is checked at the initial state and as successors are found, even if later exploration is cut off. Other temporal analyses generally need a closed graph. A reached witness remains valid with incomplete exploration; absence of one cannot be called `UNREACHABLE` until the graph closes. An unreachable query is **informational** and does not by itself cause a failing exit code. Graph, actor-creation, mailbox, domain, message-history, and evaluation limits are `INCONCLUSIVE`, not proof. Traces carry finite bad prefixes, reached witnesses, or infinite paths represented as stem plus repeating loop. Replay validates them against source, deterministic setup, bounds, allocation, and assumptions (`fml skills cli`).
