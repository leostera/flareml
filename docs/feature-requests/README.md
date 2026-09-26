# Feature request drafts

Independent, issue-ready proposals for future FlareML work. These are **drafts, not filed issues or accepted designs**. They use generic modeling examples and deliberately avoid assumptions about any particular platform. They should be evaluated against the existing single-language, fault-free execution contract in [RFD0002](../rfds/RFD0002-functions-and-actors.md); optional extensions must not silently change existing models' meaning.

Suggested order:

1. [Opt-in ambiguous outcomes for one-way interactions](01-ambiguous-outcomes.md)
2. [Explicit volatile and persistent state across restarts](02-restart-and-persistence.md)
3. [Finite maps, sets, and check-scoped identity pools](03-finite-collections.md)
4. [Readable multi-turn protocols with explicit atomic boundaries](04-multi-turn-protocols.md)
5. [Reachability and fault-path coverage reporting](05-coverage.md)
6. [Sound partial-order reduction for independent turns](06-partial-order-reduction.md)

The first two require separate semantic designs; do **not** bundle them into an implicit platform simulator. The fourth is an ergonomics request, not a request to make distributed operations atomic. The sixth requires validation against unoptimized search. Each request should be assessed with a small example, a negative/witness trace where applicable, a result/replay contract, and a concrete non-goal before implementation. No issue has been published from these drafts.
