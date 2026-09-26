# Modeling scenarios

Examples are named for the behavior or design question they model, not the language constructs they use. **Actors are FML's computational abstraction, not a deployment technology.** A modeled participant could correspond to an Erlang recursive receive loop, a Worker invocation, a containerized service, or a thread with a mailbox. That does not make their runtime semantics identical: the selected profile defines the scheduling, state, delivery and failure assumptions being checked.

## Start here: messages and state transitions

These examples use the current generic `actors-v2` profile: one `actor` form, pure initialization, state-in/state-out message handlers and explicit `send`. They assume finite FIFO mailboxes and fault-free atomic turns, not durable storage or a specific runtime.

| Example | Question | Expected result |
| --- | --- | --- |
| [counter-replies.fml](counter-replies.fml) | Can a client process the reply before the increment commits? Do submitted increments and generated replies progress under fairness? | `VERIFIED_IN_SCOPE` (exit 0), with reachable reply covers |
| [missing-reply.fml](missing-reply.fml) | Does returning the next state also send a reply? | `VIOLATED` (exit 1): a fair lasso shows the client waiting for a reply that was never sent |

```sh
cargo run -- check examples/counter-replies.fml --trace-out /tmp/counter-replies.trace.json
cargo run -- replay examples/counter-replies.fml /tmp/counter-replies.trace.json
cargo run -- check examples/missing-reply.fml --trace-out /tmp/missing-reply.trace.json # expected exit 1
cargo run -- replay examples/missing-reply.fml /tmp/missing-reply.trace.json
```

## Earlier computation profiles

These are intentionally preserved as regression examples of earlier syntax and semantics, not recommendations for new models. Renaming them does not convert synchronous calls into asynchronous sends, or make a suspending handler an atomic message turn.

| Example | Question | Profile | Expected result |
| --- | --- | --- | --- |
| [eligibility-check.fml](eligibility-check.fml) | Can a reusable eligibility policy deny every ineligible login? | `actors-v0` | `VERIFIED_IN_SCOPE` (exit 0) |
| [counter-bound.fml](counter-bound.fml) | Can two additions violate a counter bound of one? | `actors-v0` | `VIOLATED` (exit 1) |
| [forwarded-counter.fml](forwarded-counter.fml) | Does forwarding additions through an API preserve that bound? | `actors-v1` | `VIOLATED` (exit 1) |
| [isolated-accounts.fml](isolated-accounts.fml) | Do deposits to different account identities remain isolated? | `actors-v1` | `VERIFIED_IN_SCOPE` (exit 0) |
| [routed-deposits.fml](routed-deposits.fml) | Can a dispatcher route a deposit using the typed address in its input? | `actors-v1` | `VERIFIED_IN_SCOPE` (exit 0) |
| [lost-update-across-call.fml](lost-update-across-call.fml) | Can another invocation change shared state while a caller is suspended? | `actors-v1` | `VIOLATED` (exit 1): two completed increments lose one update |

## Resource-specific scenarios

These use the legacy `cf-core-v0` Worker/D1 profile. The `worker` frontend is compatibility syntax; primary-only D1 operations retain their distinct scheduling and commit boundaries. They are not implicitly mapped to generic FIFO message turns.

| Example | Question | Expected result |
| --- | --- | --- |
| [login-bug.fml](login-bug.fml) | Can a user missing from the database be authorized? | `VIOLATED` (exit 1) |
| [login-fixed.fml](login-fixed.fml) | Does denying missing users repair authorization while preserving progress? | `VERIFIED_IN_SCOPE` (exit 0), with a reachable successful login |
| [lost-update.fml](lost-update.fml) | Can separate database reads and writes lose an increment? | `VIOLATED` (exit 1) |
| [starvation.fml](starvation.fml) | Does a correct handler guarantee a response without scheduling fairness? | `VIOLATED` (exit 1): a scheduler-starvation lasso |

All verdicts concern the declared finite model and assumptions, not production conformance. Counterexamples can be saved with `--trace-out` and checked with `fml replay`. Source comments and paths have been refreshed in this naming pass; regenerate artifacts when their source hash no longer matches. See [RFD0002](../docs/rfds/RFD0002-functions-and-actors.md) for the actor model and [the acceptance checklist](../docs/rfds/RFD0002-implementation-checklist.md) for unimplemented adapters and release gates.
