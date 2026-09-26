# Requests for discussion

- [RFD0002 — Functions and actors](RFD0002-functions-and-actors.md): current language and execution contract.
- [RFD0002 implementation checklist](RFD0002-implementation-checklist.md): implemented behavior, independent evidence, and remaining acceptance work.
- [RFD0001](RFD0001-initial-language-and-model-checker.md): superseded initial design.

## Proposed roadmap — not implemented

Implement and validate these one at a time, in order. Writing the sketches does not change the current contract.

1. [RFD0003 — Nondeterministic choice and faulty links](RFD0003-nondeterministic-choice-and-faulty-links.md): finite `choose`, branch-local atomic turns, replay evidence, and lossy/duplicating-link bug/repair examples. First implementation milestone; finalize transcript and expansion-budget details before merging.
2. [RFD0004 — Bounded dynamic actor spawning](RFD0004-bounded-spawn.md): on-demand participants, finite creation pools, atomic allocation, and stable population observations. Sketch; resolve definition/instance syntax before implementation.
3. [RFD0005 — Suspension, atomic boundaries, and reentrancy](RFD0005-suspension-and-reentrancy.md): explicit segments, non-reentrant defaults, possible opt-in overlapping invocations, and a shared-memory design study. Sketch with blocking state-publication, wakeup, and fairness questions.

Each milestone requires a refined contract, checked examples, independent validation, replay/tamper coverage, and recorded evidence before proceeding. Constants and library/import design remain separate proposals, not implicit dependencies of these RFDs.

There is one supported language and engine. Historical experiments live in git history, not compatibility profiles. Proposed extensions must preserve explicit assumptions, honest finite-scope verdicts, and replayable evidence.
