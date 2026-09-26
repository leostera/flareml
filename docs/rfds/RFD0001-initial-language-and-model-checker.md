# RFD0001 — Initial language and model checker (superseded)

The initial Worker/D1-specific language and multi-profile experiments have been removed. There are no legacy users or compatibility modes to maintain.

[RFD0002](RFD0002-functions-and-actors.md) is the current language and execution contract. Its [acceptance checklist](RFD0002-implementation-checklist.md) tracks remaining validation. The original proposal and implementation remain available in git history.

Retained principles: exact finite-state exploration, explicit fairness and limits, distinct safety/reachability/liveness obligations, truthful inconclusive results, and independently validated replay. No product semantics or conformance guarantees are inherited from the superseded design.
