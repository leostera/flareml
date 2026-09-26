# Feature request: reachability and fault-path coverage in check reports

**Status:** draft; not filed. **Priority:** high for trustworthy results.

## Problem

A safety property can be verified while the behavior it was meant to protect is unreachable under the selected inputs or bounds. `reachable` properties help, but authors must anticipate and write each one. Search summaries report state counts, not which protocol branches or configured fault points were exercised.

## Minimal scenario

An actor has `Start`, `Commit`, and `Rollback` branches. A check accidentally provides no input that triggers rollback. The safety property passes; a report should make the unvisited rollback obvious without implying that simply visiting a branch proves the safety property non-vacuous.

## Requested behavior

Add optional per-check coverage of reachable actor turns, message variants, handler branches, selected fault/restart sites (if supported), and explicitly named checkpoints. Show unvisited elements separately from unsubmitted optional inputs. Where feasible, report whether each named safety antecedent occurred; preserve explicit `reachable` checks as the authoritative way to ask about a state. Offer machine-readable coverage alongside the current JSON results and source locations.

## Acceptance criteria

- The example reports rollback as unvisited; adding a triggering input marks it reached.
- Coverage differentiates enqueue, processing, and a reply actually observed. It never labels a property verified solely because an action ran.
- Coverage of a partial search is labeled **partial**; fault branches absent due to bounds/timeout must not be presented as impossible.
- Report formats and trace replay remain consistent across selected properties and checks.

## Non-goals / design questions

No code coverage of a real implementation, no inference that a production scenario is possible, and no automatic certification of non-vacuity. Define source-level branch accounting carefully when pure functions are shared across actors.
