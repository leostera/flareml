---
title: For Z3 users
description: Understand the difference between SMT satisfiability and reachable executions in FlareML.
---

Z3 is an SMT solver, not one particular modeling language. You may describe a problem with SMT-LIB or through a host-language API, then ask whether a formula is satisfiable and inspect a satisfying assignment. FlareML asks a different question: what states can a finite actor system reach by submitting inputs and processing messages?

## Assignment versus execution

A Z3 model is a valuation satisfying the constraints in a solver query. It does not, by itself, say that the valuation is reachable through a protocol. An FML `reachable P` property searches for an execution from the model's initial state to a state satisfying `P`; its witness contains the transition path. A violated `always P` property likewise returns a bad execution prefix, not just a state assignment.

You can encode bounded transition systems as SMT formulas, including a sequence of states and transition constraints. In that case the SMT encoding itself defines the transition semantics and the chosen bound. FML provides a fixed explicit-state execution model—actors, FIFO mailboxes, optional inputs, and atomic turns—instead of asking you to encode that machinery in each query.

## The value domains differ

Z3 supports symbolic reasoning over the theories and encodings you provide, such as bit-vectors, arrays, uninterpreted functions, or arithmetic. FML is deliberately finite: integer and string values come from declared finite pools; actor types and message types are closed; each check explicitly creates instances in deterministic `main` setup and sets a `spawn_bound` for every actor definition. Mailboxes have explicit capacity. FML is not an SMT front end and does not silently send expressions to Z3.

If your problem is naturally a formula over large or unbounded mathematical domains, keep using Z3. If the question is about a finite request/response protocol, a bounded population that can grow during execution, actor interleavings, FIFO ordering, or progress, FML can make setup, creation bounds, and operational assumptions explicit and return a replayable execution.

## Do not equate the verdicts

| Z3 result | FML result | Why they are not synonyms |
| --- | --- | --- |
| `sat` with a model | Sometimes a `REACHED` witness | `REACHED` requires a path from FML's initial state under its transitions, not merely a satisfying assignment. |
| `sat` for a negated invariant | Sometimes a `VIOLATED` property | The violation must be reachable in the configured FML model. |
| `unsat` | Sometimes `VERIFIED` or `UNREACHABLE` | FML's result is about its complete finite reachable graph and declared assumptions; it is not an SMT proof over an arbitrary formula. |
| Solver timeout / `unknown` | `INCONCLUSIVE` | In both cases, the requested conclusion was not established, though the underlying reasons and evidence differ. |

## A useful combination

Use Z3 for symbolic constraints that are awkward to enumerate—for example, solving a data-constraint subproblem. Then use an explicit finite abstraction in FML when you need to explore protocol behavior. Document the abstraction boundary: FML validates the transition system you wrote, not the correctness of an independent SMT encoding or the full unbounded data domain.

See [checks and bounds](/reference/checks/) for finite pools and exploration limits, and the [execution model](/guide/execution-model/) for the transitions that FML explores.
