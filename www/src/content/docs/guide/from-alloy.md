---
title: For Alloy users
description: Compare Alloy's relational instance analysis with FlareML's finite actor-state exploration.
---

Alloy and FlareML both work with finite models, but they answer different kinds of questions. Alloy is a relational modeling language whose analyzer searches for bounded instances satisfying or violating formulas. Alloy 6 also supports mutable relations and temporal traces. FlareML instead defines an operational system of actors, messages, and scheduled turns, then explores its reachable states.

## Relational instance or execution state?

An Alloy scope bounds the number of atoms available for signatures in an instance. A satisfying instance answers a structural question about relations; with temporal modeling, Alloy can also search traces through mutable state. In FML, a check starts with no actor instances and runs deterministic `main` setup. `spawn_bound A = N` caps how many instances of actor definition A setup and handlers may create over the whole run; data domains bound values, and `mailbox_bound` limits pending messages per created address. The checker explores states connected by explicit setup, input, and actor-turn transitions.

These bounds are not interchangeable. A scope of three Alloy atoms does not directly correspond to an FML `domain Int = 0..2`, `spawn_bound A = 3`, or mailbox capacity of three. FML `instances(A)` specifications range over stable potential slots, including unborn instances, so account for those slots in quantified properties. Choose the FML bounds from the system question, and report them with the result.

## Translate the question, not the command

| Alloy question | Possible FML formulation | Caveat |
| --- | --- | --- |
| Does a static structure satisfy a relational constraint? | Often keep this in Alloy | FML is not a general relational constraint solver. Its properties inspect states of an operational actor model. |
| Can some execution reach a bad state? | `reachable P` | This asks whether a state satisfying `P` is reachable through FML transitions—not whether `P` is satisfiable as a standalone formula. |
| Does an invariant hold throughout executions? | `always P` | This ranges over FML's finite actor-runtime graph and declared assumptions. |
| Find a counterexample to an Alloy assertion | A violated FML `always P` may be analogous | The assertion must be restated over FML state and execution semantics; it is not automatically translated. |

One command-name trap: Alloy's `check` searches for a counterexample to an assertion. In FML, `check` names a finite experiment—its domains, inputs, bounds, and fairness. FML claims are declared separately with `property`.

## When FML is a good fit

Use FML when the question depends on who processes which message, per-recipient FIFO order, state changes between turns, optional environment inputs, bounded creation, or scheduling between participants. `main` creates the initial actors; handlers may create later actors using the same per-type lifetime bounds. An FML handler commits one actor's new state, new instances, and outgoing messages atomically. Messages to other actors run in later turns. If a real operation has an intermediate stage where another actor can run, represent that stage with another message/turn.

Keep Alloy for questions naturally expressed as relations, structural constraints, or bounded configurations. You can use both: Alloy can check whether a topology or configuration is well-formed, while FML explores a protocol running over a chosen finite configuration.

## Example direction

Suppose an Alloy model asks whether two requests can both be accepted for one remaining item. In FML, make the stock manager an actor, represent the buyers' requests and replies as typed messages, and model the check/offer/reserve steps as separate turns. Then ask `always` that stock never becomes negative or `reachable` whether both buyers receive acceptance. Those properties only mean what your chosen turns, inputs, and finite bounds say they mean.

The [inventory reservation bug and repair](/examples/) are examples of this protocol-style question. Read the [actor reference](/reference/actors/) before deciding which steps should be atomic.
