---
title: Examples
description: Current-version FML examples with explicit setup, bounded creation, and tested verdicts.
---

These examples use the current explicit-population model: actor declarations create no instances; each check's `main` creates its initial participants and every check declares a `spawn_bound` for each actor definition. Examples have tested verdicts and replayable witnesses. Start with the [sequential workflow](https://github.com/leostera/flareml/blob/main/examples/sequential-workflow.fml), then compare [lost updates](https://github.com/leostera/flareml/blob/main/examples/lost-update.fml) with [atomic increments](https://github.com/leostera/flareml/blob/main/examples/atomic-increments.fml).

| Example | Question | Result |
| --- | --- | --- |
| [Explicit startup](https://github.com/leostera/flareml/blob/main/examples/explicit-startup.fml) | Does deterministic setup enqueue startup work before exploration? | Verified; completion reachable |
| [Sequential workflow](https://github.com/leostera/flareml/blob/main/examples/sequential-workflow.fml) | Does one participant move through preparation and commit steps? | Verified; intermediate states reachable |
| [Eligibility check](https://github.com/leostera/flareml/blob/main/examples/eligibility-check.fml) | Can a policy deny a request that does not meet its conditions? | Verified |
| [Link shortener](/guide/link-shortener/) | Can a client shorten and resolve a destination, and does the redirect match what was saved? | Verified |
| [Spawned workers](https://github.com/leostera/flareml/blob/main/examples/spawn-workers.fml) | Do two optional jobs create distinct workers, preserve correlations, and finish? | Verified; both jobs can finish |
| [Choice and spawn](https://github.com/leostera/flareml/blob/main/examples/spawn-choice-workers.fml) | Do handler choice branches reserve fresh identities independently? | Verified; two-worker creation reachable |
| [Faulty link with loss](https://github.com/leostera/flareml/blob/main/examples/faulty-link-loss.fml) | Can a fairly processed link request still fail to arrive? | Violated: explicit drop; delivery also reachable |
| [Faulty link duplication bug](https://github.com/leostera/flareml/blob/main/examples/faulty-link-duplicate-bug.fml) | Can an explicit duplicate-delivery choice make a receiver apply one request twice? | Violated: duplicate application |
| [Faulty link duplication repair](https://github.com/leostera/flareml/blob/main/examples/faulty-link-duplicate-fixed.fml) | Does request-keyed deduplication prevent duplicate application? | Verified in scope; loss remains possible |
| [Counter replies](https://github.com/leostera/flareml/blob/main/examples/counter-replies.fml) | Are replies processed after commit, and does generated work progress fairly? | Verified; reply reachable |
| [Missing reply](https://github.com/leostera/flareml/blob/main/examples/missing-reply.fml) | Does returning state implicitly send a reply? | Violated: fair missing-reply lasso |
| [Routed deposits](https://github.com/leostera/flareml/blob/main/examples/routed-deposits.fml) | Do transferred actor references route deposits to isolated accounts? | Verified; both deposits reachable |
| [Lost update](https://github.com/leostera/flareml/blob/main/examples/lost-update.fml) | Can two read-modify-write clients lose an update across protocol turns? | Violated: both clients finish but storage contains one increment |
| [Atomic increments](https://github.com/leostera/flareml/blob/main/examples/atomic-increments.fml) | Does making increment a single storage turn repair the lost update? | Verified; both acknowledgments reachable |
| [Inventory reservation bug](https://github.com/leostera/flareml/blob/main/examples/inventory-reservation-bug.fml) | Can two buyers both receive the last item after separate availability checks? | Violated: both accepted |
| [Inventory reservation repair](https://github.com/leostera/flareml/blob/main/examples/inventory-reservation-fixed.fml) | Does reserving stock when making the offer prevent overselling? | Verified; both buyers can finish |
| [Payment idempotency bug](https://github.com/leostera/flareml/blob/main/examples/payment-idempotency-bug.fml) | Can duplicate deliveries charge the same payment key twice? | Violated: duplicate charge |
| [Payment idempotency repair](https://github.com/leostera/flareml/blob/main/examples/payment-idempotency-fixed.fml) | Does key-based deduplication at the modeled charge boundary prevent duplicate charges? | Verified; distinct payments charge and the duplicate can finish |

Copy a model from the repository and check it with the installed CLI:

```sh
fml check examples/spawn-workers.fml --trace-out /tmp/workers.json
fml replay examples/spawn-workers.fml /tmp/workers.json
```

Each check also saves its exact source, configuration, report, and all available witnesses in `.fml/runs/<run-id>/`; see the [CLI manual](/reference/cli/) to replay against that saved snapshot. Actors are a computational abstraction, not deployment categories. Storage examples are protocol models, **not database product adapters**. If a real operation can interleave, split it into separate turns. A passing model does not prove implementation conformance.

The faulty-link models use handler-local `choose`: every compatible outcome is explored, but none is forced by fairness. The engine does not lose committed sends; a link actor explicitly decides whether to forward. The repair proves duplicate-application safety, **not** eventual delivery or durability. Likewise, the inventory and payment repairs depend on their stated atomic boundaries and omit real-world concerns such as expiry, cancellation, external APIs, and crashes. Model your actual assumptions before drawing conclusions about a deployed system.
