# Requests for discussion

## Implemented

- [RFD0002 — Functions and actors](RFD0002-functions-and-actors.md): current language and execution contract, including choice and bounded spawn.
- [RFD0002 implementation checklist](RFD0002-implementation-checklist.md): independent evidence and remaining core stabilization work.
- [RFD0003 — Nondeterministic choice and faulty links](RFD0003-nondeterministic-choice-and-faulty-links.md): finite `choose`, branch-local atomic turns, choice replay evidence, faulty-link examples, and recorded validation/measurements.
- [RFD0004 — Explicit populations and bounded actor spawning](RFD0004-bounded-spawn.md): actor definitions, deterministic `check.main`, shared lifetime creation pools, atomic allocation, stable future-instance observations, and format-8 replay.

## Next sketches — not implemented

Implement and validate these one at a time, in order. Writing the sketches does not change the current contract.

1. [RFD0005 — Suspension, atomic boundaries, and reentrancy](RFD0005-suspension-and-reentrancy.md): explicit segments, non-reentrant defaults, possible opt-in overlapping invocations, and a shared-memory design study. Blocking state-publication, wakeup, and fairness questions remain.

Each milestone requires a refined contract, checked examples, independent validation, replay/tamper coverage, and recorded evidence before proceeding. Constants and library/import design remain separate proposals, not implicit dependencies of these RFDs.

[RFD0001](RFD0001-initial-language-and-model-checker.md) is the superseded initial design. There is one supported language and engine. Historical experiments live in git history, not compatibility profiles. Extensions must preserve explicit assumptions, honest finite-scope verdicts, and replayable evidence.
