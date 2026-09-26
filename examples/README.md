# Modeling scenarios

All examples use the same current language and FIFO/atomic-turn execution contract. No semantics selector or legacy syntax exists. Bindings and intermediate statements require semicolons (`send(target, message); next_state`); the final unterminated expression supplies the block's value.

| Example | Question | Expected result |
| --- | --- | --- |
| [sequential-workflow.fml](sequential-workflow.fml) | Does a single participant progress through observable preparation/commit steps? | Verified; intermediate states reachable |
| [eligibility-check.fml](eligibility-check.fml) | Can a pure policy deny an ineligible request? | Verified |
| [counter-replies.fml](counter-replies.fml) | Are replies processed after commit, and does generated work progress fairly? | Verified; reply reachable |
| [missing-reply.fml](missing-reply.fml) | Does returning state implicitly send a reply? | Violated: fair missing-reply lasso |
| [routed-deposits.fml](routed-deposits.fml) | Do transferable addresses route deposits to isolated keyed accounts? | Verified; both deposits reachable |
| [lost-update.fml](lost-update.fml) | Can two read-modify-write clients lose an update across protocol turns? | Violated: both clients finish but storage contains one increment |
| [atomic-increments.fml](atomic-increments.fml) | Does making increment a single storage turn repair the lost update? | Verified; both acknowledgments reachable |

| [inventory-reservation-bug.fml](inventory-reservation-bug.fml) | Can two buyers both receive the last item after separate availability checks? | Violated: both accepted |
| [inventory-reservation-fixed.fml](inventory-reservation-fixed.fml) | Does reserving stock when making the offer prevent overselling? | Verified; both buyers can finish |
| [payment-idempotency-bug.fml](payment-idempotency-bug.fml) | Can duplicate deliveries charge the same payment key twice? | Violated: duplicate charge |
| [payment-idempotency-fixed.fml](payment-idempotency-fixed.fml) | Does key-based deduplication at the charge boundary prevent duplicate charges? | Verified; distinct payments charge and the duplicate can finish |

| [faulty-link-loss.fml](faulty-link-loss.fml) | Can a fairly processed link request still fail to arrive? | Violated: explicit drop, fair lasso; delivery also reachable |
| [faulty-link-duplicate-bug.fml](faulty-link-duplicate-bug.fml) | Can an explicit duplicating link cause two applications of one request? | Violated: duplicate application |
| [faulty-link-duplicate-fixed.fml](faulty-link-duplicate-fixed.fml) | Does request-keyed deduplication absorb the duplicate? | Verified in scope; delivery and duplicate completion reachable |

```sh
cargo run --locked -- check examples/counter-replies.fml --trace-out /tmp/replies.json
cargo run --locked -- replay examples/counter-replies.fml /tmp/replies.json
cargo run --locked -- check examples/lost-update.fml --trace-out /tmp/lost.json # exit 1 by design
cargo run --locked -- replay examples/lost-update.fml /tmp/lost.json
```

Actors are a computational abstraction, not deployment categories. The storage examples are abstract protocols, **not D1 or database product adapters**. If the real system exposes intermediate effects, the model must split them into separate turns. A passing model does not prove implementation conformance.

The inventory repair assumes buyers commit only after an offer; it does not model reservation expiry or cancellations. The payment repair makes the ledger update and charge one atomic processor turn. It does **not** establish that an application-side cache can atomically deduplicate a separate external payment API. Duplicate payment deliveries are explicit input slots, not hidden engine behavior. Extend these models with your actual retry, crash, and external-side-effect boundaries before drawing conclusions about a real system.

The faulty-link models use `choose` inside an atomic link turn. Every alternative is explored; none is forced by fairness. The engine does not lose committed sends: the link processes an incoming packet and may choose not to forward it. The repair proves duplicate-application safety, **not** eventual delivery or durability. These immediate-forwarding links do not model reordering; that requires an explicit packet buffer. `tests/faulty_links.rs` selects each obligation independently so early safety failures cannot hide another example's intended outcome.

Every check automatically saves a run bundle under `.fml/runs/`, including the source snapshot and all available witnesses; replay against that snapshot after editing the original model.

Every `.fml` file here must be listed in `tests/examples.rs`, which checks its verdict and replays every generated witness. Regenerate artifacts when source changes.
