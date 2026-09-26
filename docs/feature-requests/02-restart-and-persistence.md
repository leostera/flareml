# Feature request: explicit restart and persistent-versus-volatile state

**Status:** draft; not filed. **Priority:** high for recovery protocols.

## Problem

An FML actor currently has state but no crash, restart, or persistence semantics. To investigate an operation that commits an external effect and then loses its progress marker, model authors must hand-build another scheduler and two copies of state. They can easily hide the ordering that matters.

## Minimal scenario

A job records `Started`, asks another actor to apply effect `X`, and then records `Completed`. Explore a restart after `X` commits but before `Completed` is persisted. The job runs again. Can `X` be applied twice? Contrast a state machine that records an idempotency key with one that does not. Permit a second attempt to start while the first is pending as a **separate, explicitly selected hypothesis**, not a baked-in scheduler guarantee.

## Requested behavior

Offer an opt-in way to distinguish retained state from volatile execution state, define a named restart point, and choose a small number of restarts per check. State precisely what happens to in-flight messages and queued outgoing effects during restart. Let model authors distinguish a committed checkpoint from an external effect; never imply they commit atomically. Label restarts and retained/lost values in counterexample traces and validate them in replay.

## Acceptance criteria

- An effect-before-checkpoint scenario yields a reachable retry and a safety violation for a non-idempotent example.
- Selecting no restart preserves current fault-free behavior exactly.
- Models can compare serialized and overlapping attempts without claiming either is the default execution contract.
- Restart and state-retention assumptions appear in check output; cutoffs are inconclusive.

## Non-goals / design questions

No automatic durable storage implementation, process supervision, timers, exactly-once guarantee, or compatibility claim with a particular runtime. It may be better to ship a well-specified reusable library and examples before adding a language primitive. Transport ambiguity is a related but independent request.
